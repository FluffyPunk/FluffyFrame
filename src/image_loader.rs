//! `http(s)://` bytes loader for item art, backed by the on-disk
//! [`image_cache`](wfhelper_core::services::image_cache).
//!
//! Replaces egui_extras' `http` loader, which kept images only in memory (so
//! every launch re-downloaded everything) and spawned a thread per request.
//! Cached images are read straight from disk; misses go to a small pool of
//! workers sharing the core HTTP client's connection pool.

use std::collections::HashMap;
use std::sync::Arc;
use std::task::Poll;

use crossbeam_channel::Sender;
use egui::load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError};
use egui::mutex::Mutex;
use wfhelper_core::services::image_cache;

const DOWNLOAD_WORKERS: usize = 8;

type Entry = Poll<Result<Arc<[u8]>, String>>;

struct CachedHttpLoader {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    downloads: Sender<String>,
}

pub fn install(ctx: &egui::Context) {
    let entries: Arc<Mutex<HashMap<String, Entry>>> = Default::default();
    let (downloads, queue) = crossbeam_channel::unbounded::<String>();
    for i in 0..DOWNLOAD_WORKERS {
        let (ctx, entries, queue) = (ctx.clone(), entries.clone(), queue.clone());
        std::thread::Builder::new()
            .name(format!("image-dl-{i}"))
            .spawn(move || {
                for url in queue {
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
                        let _ = self.downloads.send(uri.to_owned());
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
