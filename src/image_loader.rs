//! `http(s)://` bytes loader for item art, backed by the on-disk
//! [`image_cache`](wfhelper_core::services::image_cache).
//!
//! Replaces egui_extras' `http` loader, which kept images only in memory (so
//! every launch re-downloaded everything) and spawned a thread per request.
//! Cached images are read straight from disk; misses go to a small pool of
//! workers sharing the core HTTP client's connection pool.
//!
//! Downloads and decodes run newest-first: while scrolling, the cards now on
//! screen are served before the ones scrolled past.

use std::collections::HashMap;
use std::sync::Arc;
use std::task::Poll;

use egui::load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError};
use egui::mutex::Mutex;
use wfhelper_core::services::image_cache;

const DOWNLOAD_WORKERS: usize = 8;

/// Blocking work stack: `pop` returns the most recently pushed job.
struct Lifo<T> {
    jobs: std::sync::Mutex<Vec<T>>,
    ready: std::sync::Condvar,
}

impl<T> Lifo<T> {
    fn new() -> Arc<Self> {
        Arc::new(Self { jobs: std::sync::Mutex::new(Vec::new()), ready: std::sync::Condvar::new() })
    }

    fn push(&self, job: T) {
        self.jobs.lock().unwrap().push(job);
        self.ready.notify_one();
    }

    fn pop(&self) -> T {
        let mut jobs = self.jobs.lock().unwrap();
        loop {
            if let Some(job) = jobs.pop() {
                return job;
            }
            jobs = self.ready.wait(jobs).unwrap();
        }
    }
}

type Entry = Poll<Result<Arc<[u8]>, String>>;

struct CachedHttpLoader {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    downloads: Arc<Lifo<String>>,
}

pub fn install(ctx: &egui::Context) {
    let entries: Arc<Mutex<HashMap<String, Entry>>> = Default::default();
    let downloads = Lifo::<String>::new();
    for i in 0..DOWNLOAD_WORKERS {
        let (ctx, entries, queue) = (ctx.clone(), entries.clone(), downloads.clone());
        std::thread::Builder::new()
            .name(format!("image-dl-{i}"))
            .spawn(move || {
                loop {
                    let url = queue.pop();
                    let result = image_cache::fetch(&url).map(Arc::from).map_err(|err| {
                        log::warn!("image download failed: {err:#}");
                        format!("Failed to load {url:?}")
                    });
                    // Skip if `forget` dropped the entry meanwhile. Never lock the
                    // Context while holding our lock (egui's loader rule).
                    let stored = match entries.lock().get_mut(&url) {
                        Some(entry) => {
                            *entry = Poll::Ready(result);
                            true
                        }
                        None => false,
                    };
                    if stored {
                        ctx.request_repaint();
                    }
                }
            })
            .expect("spawn image download worker");
    }
    ctx.add_bytes_loader(Arc::new(CachedHttpLoader { entries, downloads }));
}

impl BytesLoader for CachedHttpLoader {
    fn id(&self) -> &str {
        egui::generate_loader_id!(CachedHttpLoader)
    }

    fn load(&self, _ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if !(uri.starts_with("https://") || uri.starts_with("http://")) {
            return Err(LoadError::NotSupported);
        }
        let mut entries = self.entries.lock();
        let entry = match entries.get(uri) {
            Some(entry) => entry.clone(),
            None => {
                let entry = match image_cache::read(uri) {
                    Some(bytes) => Poll::Ready(Ok(Arc::from(bytes))),
                    None => {
                        self.downloads.push(uri.to_owned());
                        Poll::Pending
                    }
                };
                entries.insert(uri.to_owned(), entry.clone());
                entry
            }
        };
        match entry {
            // No mime: the image loaders sniff the format from the bytes.
            Poll::Ready(Ok(bytes)) => Ok(BytesPoll::Ready { size: None, bytes: Bytes::Shared(bytes), mime: None }),
            Poll::Ready(Err(err)) => Err(LoadError::Loading(err)),
            Poll::Pending => Ok(BytesPoll::Pending { size: None }),
        }
    }

    fn forget(&self, uri: &str) {
        self.entries.lock().remove(uri);
    }

    fn forget_all(&self) {
        self.entries.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.entries
            .lock()
            .values()
            .map(|e| match e {
                Poll::Ready(Ok(bytes)) => bytes.len(),
                Poll::Ready(Err(err)) => err.len(),
                Poll::Pending => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.entries.lock().values().any(Poll::is_pending)
    }
}

// ---------------------------------------------------------------------------
// Thumbnails

/// Longest side of a decoded thumbnail, in points. Item art ships at up to
/// 512×512 (1 MiB as RGBA) but is drawn at ~100 points.
const THUMB_POINTS: f32 = 128.0;
const THUMB_MAX_PX: u32 = 256;
/// Decoded thumbnails kept before least-recently-drawn ones are evicted.
const THUMB_BUDGET_BYTES: usize = 48 * 1024 * 1024;
/// Never evict what was drawn this recently (in passes).
const THUMB_KEEP_PASSES: u64 = 3;
/// A queued decode is skipped if its card has not been drawn for this many
/// passes (scrolled away); drawing it again re-queues it.
const THUMB_STALE_PASSES: u64 = 10;

struct Thumb {
    state: Poll<Result<Arc<egui::ColorImage>, String>>,
    last_used: u64,
}

#[derive(Default)]
struct ThumbState {
    thumbs: HashMap<String, Thumb>,
    pass: u64,
    /// Evicted URIs whose textures and bytes the app still has to release.
    evicted: Vec<String>,
}

/// Decodes remote item art straight to display-sized thumbnails and drops the
/// downloaded bytes afterwards, so memory stays flat while browsing thousands
/// of items. Local (`bytes://`) images go to egui_extras' loaders as before.
/// (uri, downloaded bytes, max thumbnail side in pixels)
type DecodeJob = (String, Arc<[u8]>, u32);

struct ThumbnailLoader {
    state: Arc<Mutex<ThumbState>>,
    decode: Arc<Lifo<DecodeJob>>,
}

static THUMBS: std::sync::OnceLock<Arc<Mutex<ThumbState>>> = std::sync::OnceLock::new();

/// Strips egui's animation frame suffix (`…webp#0`).
fn base_uri(uri: &str) -> &str {
    match uri.rsplit_once('#') {
        Some((base, frame)) if frame.chars().all(|c| c.is_ascii_digit()) => base,
        _ => uri,
    }
}

fn decode_thumbnail(bytes: &[u8], max_px: u32) -> Result<egui::ColorImage, String> {
    let image = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    let source = egui::vec2(image.width() as f32, image.height() as f32);
    let image = if image.width().max(image.height()) > max_px { image.thumbnail(max_px, max_px) } else { image };
    let rgba = image.to_rgba8();
    let mut out = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], rgba.as_raw());
    // Layout keeps using the original size; only the texels shrink.
    out.source_size = source;
    Ok(out)
}

pub fn install_thumbnails(ctx: &egui::Context) {
    let state = THUMBS.get_or_init(Default::default).clone();
    let decode = Lifo::<DecodeJob>::new();
    // Leave a core for the UI thread.
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1)).clamp(2, 6);
    for i in 0..workers {
        let (ctx, state, jobs) = (ctx.clone(), state.clone(), decode.clone());
        std::thread::Builder::new()
            .name(format!("image-decode-{i}"))
            .spawn(move || {
                loop {
                    let (uri, bytes, max_px) = jobs.pop();
                    // Scrolled away before its turn: drop it; drawing the card
                    // again re-queues it (the bytes stay in the disk cache).
                    {
                        let mut s = state.lock();
                        let pass = s.pass;
                        if s.thumbs.get(&uri).is_some_and(|t| t.last_used + THUMB_STALE_PASSES < pass) {
                            s.thumbs.remove(&uri);
                            continue;
                        }
                    }
                    let result = decode_thumbnail(&bytes, max_px).map(Arc::new);
                    drop(bytes);
                    if let Err(err) = &result {
                        log::warn!("image decode failed for {uri}: {err}");
                    }
                    let stored = match state.lock().thumbs.get_mut(&uri) {
                        Some(t) => {
                            t.state = Poll::Ready(result);
                            true
                        }
                        None => false,
                    };
                    // The downloaded bytes are no longer needed in memory (the
                    // disk cache keeps them). Not under our lock.
                    for loader in ctx.loaders().bytes.lock().iter() {
                        loader.forget(&uri);
                    }
                    if stored {
                        ctx.request_repaint();
                    }
                }
            })
            .expect("spawn image decode worker");
    }
    ctx.add_image_loader(Arc::new(ThumbnailLoader { state, decode }));
}

/// Frees textures of evicted thumbnails; call once per frame outside egui's
/// loader callbacks (they must not re-enter the loaders).
pub fn release_evicted(ctx: &egui::Context) {
    let Some(state) = THUMBS.get() else { return };
    let evicted = std::mem::take(&mut state.lock().evicted);
    for uri in evicted {
        ctx.forget_image(&uri);
    }
}

impl egui::load::ImageLoader for ThumbnailLoader {
    fn id(&self) -> &str {
        egui::generate_loader_id!(ThumbnailLoader)
    }

    fn load(&self, ctx: &egui::Context, uri: &str, _: egui::SizeHint) -> egui::load::ImageLoadResult {
        use egui::load::ImagePoll;
        if !(uri.starts_with("https://") || uri.starts_with("http://")) {
            return Err(LoadError::NotSupported);
        }
        let key = base_uri(uri);
        {
            let mut state = self.state.lock();
            let pass = state.pass;
            if let Some(t) = state.thumbs.get_mut(key) {
                t.last_used = pass;
                return match &t.state {
                    Poll::Ready(Ok(image)) => Ok(ImagePoll::Ready { image: image.clone() }),
                    Poll::Ready(Err(err)) => Err(LoadError::Loading(err.clone())),
                    Poll::Pending => Ok(ImagePoll::Pending { size: None }),
                };
            }
        }
        // Not under our lock: this calls into the bytes loaders.
        match ctx.try_load_bytes(key) {
            Ok(BytesPoll::Ready { bytes, .. }) => {
                let max_px = ((THUMB_POINTS * ctx.pixels_per_point()).round() as u32).clamp(64, THUMB_MAX_PX);
                let mut state = self.state.lock();
                let pass = state.pass;
                state.thumbs.insert(key.to_owned(), Thumb { state: Poll::Pending, last_used: pass });
                self.decode.push((key.to_owned(), Arc::from(&bytes[..]), max_px));
                Ok(ImagePoll::Pending { size: None })
            }
            Ok(BytesPoll::Pending { size }) => Ok(ImagePoll::Pending { size }),
            Err(err) => Err(err),
        }
    }

    fn forget(&self, uri: &str) {
        self.state.lock().thumbs.remove(base_uri(uri));
    }

    fn forget_all(&self) {
        self.state.lock().thumbs.clear();
    }

    fn end_pass(&self, pass_index: u64) {
        let mut state = self.state.lock();
        state.pass = pass_index;
        let size = |t: &Thumb| match &t.state {
            Poll::Ready(Ok(image)) => image.pixels.len() * 4,
            _ => 0,
        };
        let mut total: usize = state.thumbs.values().map(size).sum();
        if total <= THUMB_BUDGET_BYTES {
            return;
        }
        // Evict least recently drawn first, sparing anything on screen now.
        let mut victims: Vec<(u64, String, usize)> = state
            .thumbs
            .iter()
            .filter(|(_, t)| t.last_used + THUMB_KEEP_PASSES < pass_index && size(t) > 0)
            .map(|(uri, t)| (t.last_used, uri.clone(), size(t)))
            .collect();
        victims.sort_unstable_by_key(|v| v.0);
        // Free down to 3/4 of the budget so eviction does not run every frame.
        for (_, uri, bytes) in victims {
            if total <= THUMB_BUDGET_BYTES * 3 / 4 {
                break;
            }
            state.thumbs.remove(&uri);
            state.evicted.push(uri);
            total -= bytes;
        }
    }

    fn byte_size(&self) -> usize {
        self.state
            .lock()
            .thumbs
            .values()
            .map(|t| match &t.state {
                Poll::Ready(Ok(image)) => image.pixels.len() * 4,
                _ => 0,
            })
            .sum()
    }

    fn has_pending(&self) -> bool {
        self.state.lock().thumbs.values().any(|t| t.state.is_pending())
    }
}
