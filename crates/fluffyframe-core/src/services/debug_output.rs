//! Windows: the game's log lines as it writes them. Warframe sends every
//! EE.log line through `OutputDebugStringA` too, and EE.log itself reaches the
//! disk in batches ~10 s late. Without a debugger attached, Windows hands
//! debug strings to whoever listens on the session's `DBWIN_BUFFER` (what
//! Sysinternals DebugView reads): a 4 KB shared buffer, the sender's process
//! id then the text, passed with two events.
//!
//! While a listener exists, every `OutputDebugString` call in the session
//! waits (up to 10 s) for it to free the buffer, so the capture thread does
//! nothing but copy the text out and free it again. When another listener
//! already owns the buffer (DebugView open), the capture stays off rather
//! than fight over it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Memory::{CreateFileMappingW, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile};
use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject};

/// The shared buffer: a `u32` process id, then up to 4092 bytes of text.
const BUFFER_SIZE: usize = 4096;
/// How often the capture thread checks whether to stop.
const WAKE: Duration = Duration::from_millis(250);

/// A running capture; dropping it stops listening.
pub struct Capture {
    rx: Receiver<(u32, String)>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    /// Starts listening, or `None` when another listener owns the buffer or
    /// the system refuses it (the caller keeps reading EE.log alone).
    pub fn start() -> Option<Self> {
        let channel = Channel::open().map_err(|e| log::info!("game debug output not captured: {e}")).ok()?;
        let (tx, rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("game-debug-output".into())
            .spawn(move || channel.run(&tx, &flag))
            .map_err(|e| log::warn!("game debug output thread: {e}"))
            .ok()?;
        log::info!("capturing the game's debug output");
        Some(Self { rx, stop, thread: Some(thread) })
    }

    /// The next debug string (sender's process id, text), waiting up to `wait`.
    pub fn recv(&self, wait: Duration) -> Result<(u32, String), RecvTimeoutError> {
        self.rx.recv_timeout(wait)
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// `"DBWIN_BUFFER"` as a NUL-terminated wide string.
fn wide(name: &str) -> Vec<u16> {
    name.encode_utf16().chain([0]).collect()
}

/// The buffer, its view and the two events, closed on drop.
struct Channel {
    buffer_ready: HANDLE,
    data_ready: HANDLE,
    mapping: HANDLE,
    view: MEMORY_MAPPED_VIEW_ADDRESS,
}

// The handles and view are only used by the capture thread they move to.
unsafe impl Send for Channel {}

impl Channel {
    fn open() -> Result<Self, String> {
        unsafe {
            // Auto-reset events: each signal wakes one waiter once.
            let buffer_ready = CreateEventW(std::ptr::null(), 0, 0, wide("DBWIN_BUFFER_READY").as_ptr());
            if buffer_ready.is_null() {
                return Err(format!("creating DBWIN_BUFFER_READY (error {})", GetLastError()));
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(buffer_ready);
                return Err("another program (DebugView?) is already listening".into());
            }
            let mut channel = Self { buffer_ready, data_ready: std::ptr::null_mut(), mapping: std::ptr::null_mut(), view: Default::default() };
            channel.data_ready = CreateEventW(std::ptr::null(), 0, 0, wide("DBWIN_DATA_READY").as_ptr());
            if channel.data_ready.is_null() {
                return Err(format!("creating DBWIN_DATA_READY (error {})", GetLastError()));
            }
            channel.mapping =
                CreateFileMappingW(INVALID_HANDLE_VALUE, std::ptr::null(), PAGE_READWRITE, 0, BUFFER_SIZE as u32, wide("DBWIN_BUFFER").as_ptr());
            if channel.mapping.is_null() {
                return Err(format!("creating DBWIN_BUFFER (error {})", GetLastError()));
            }
            channel.view = MapViewOfFile(channel.mapping, FILE_MAP_READ, 0, 0, BUFFER_SIZE);
            if channel.view.Value.is_null() {
                return Err(format!("mapping DBWIN_BUFFER (error {})", GetLastError()));
            }
            Ok(channel)
        }
    }

    /// Frees the buffer, waits for a string, copies it out, and again, until `stop`.
    fn run(self, tx: &Sender<(u32, String)>, stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) {
            unsafe {
                SetEvent(self.buffer_ready);
                if WaitForSingleObject(self.data_ready, WAKE.as_millis() as u32) != WAIT_OBJECT_0 {
                    continue;
                }
                let base = self.view.Value.cast::<u8>().cast_const();
                let pid = std::ptr::read_unaligned(base.cast::<u32>());
                let text = std::slice::from_raw_parts(base.add(4), BUFFER_SIZE - 4);
                let len = text.iter().position(|&b| b == 0).unwrap_or(text.len());
                let text = String::from_utf8_lossy(&text[..len]).into_owned();
                if tx.send((pid, text)).is_err() {
                    return;
                }
            }
        }
    }
}

impl Drop for Channel {
    fn drop(&mut self) {
        unsafe {
            if !self.view.Value.is_null() {
                UnmapViewOfFile(self.view);
            }
            for handle in [self.mapping, self.data_ready, self.buffer_ready] {
                if !handle.is_null() {
                    CloseHandle(handle);
                }
            }
        }
    }
}
