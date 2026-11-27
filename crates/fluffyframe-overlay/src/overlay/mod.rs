//! A click-through panel drawn over the game, in the top-right corner of a
//! monitor. It takes no focus and no input, and window managers don't place
//! it, so tiling ones leave the layout alone:
//!
//! - Wayland: a wlr-layer-shell surface on the overlay layer (Hyprland, Sway,
//!   KDE, river, niri, COSMIC…); above fullscreen windows.
//! - X11, and Wayland desktops without layer-shell (GNOME, through XWayland):
//!   an override-redirect window with an empty input shape.
//! - Windows: a layered, topmost, transparent tool window.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tiny_skia::Pixmap;

mod render;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

pub use render::Fonts;

/// Space between the panel and the monitor's edges.
const MARGIN: i32 = 32;
/// How often the platform's events are handled between commands.
const TICK: Duration = Duration::from_millis(50);

/// What the overlay shows: a title and lines under it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Panel {
    pub title: String,
    pub lines: Vec<String>,
}

enum Cmd {
    Show { monitor: Option<String>, panel: Panel, hide_after: Option<Duration> },
    Hide,
}

/// Handle to the overlay thread; dropping it closes the overlay.
pub struct Overlay {
    tx: Sender<Cmd>,
}

impl Overlay {
    /// Starts the overlay thread. `fonts` are tried in order per character.
    pub fn start(fonts: Vec<&'static [u8]>) -> Self {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("overlay".into()).spawn(move || run(&Fonts::new(&fonts), &rx));
        if let Err(e) = spawned {
            log::warn!("overlay thread: {e}");
        }
        Self { tx }
    }

    /// Shows `panel` on the monitor named `monitor` (or the first one).
    pub fn show(&self, monitor: Option<&str>, panel: Panel) {
        let _ = self.tx.send(Cmd::Show { monitor: monitor.map(str::to_string), panel, hide_after: None });
    }

    /// Shows `panel` like [`show`](Self::show), hidden again after `duration`
    /// unless something else is shown first.
    pub fn show_for(&self, monitor: Option<&str>, panel: Panel, duration: Duration) {
        let _ = self.tx.send(Cmd::Show { monitor: monitor.map(str::to_string), panel, hide_after: Some(duration) });
    }

    pub fn hide(&self) {
        let _ = self.tx.send(Cmd::Hide);
    }
}

/// A platform's way of putting pixels on screen.
trait Backend {
    /// Shows `pixels` (premultiplied RGBA) at the top-right of `monitor`.
    fn show(&mut self, monitor: Option<&str>, pixels: &Pixmap) -> anyhow::Result<()>;
    fn hide(&mut self);
    /// Handles pending platform events (redraws, configure…).
    fn pump(&mut self) -> anyhow::Result<()>;
}

fn backend() -> anyhow::Result<Box<dyn Backend>> {
    #[cfg(target_os = "linux")]
    {
        use crate::monitors::{Session, session};
        if session() == Session::Wayland {
            match wayland::LayerOverlay::new() {
                Ok(b) => return Ok(Box::new(b)),
                Err(e) => log::info!("no layer-shell overlay ({e:#}); trying X11"),
            }
        }
        if std::env::var_os("DISPLAY").is_some() {
            return Ok(Box::new(x11::X11Overlay::new()?));
        }
        anyhow::bail!("no display for the overlay")
    }
    #[cfg(windows)]
    {
        Ok(Box::new(windows::WinOverlay::new()?))
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        anyhow::bail!("the overlay is not supported on this platform")
    }
}

fn run(fonts: &Fonts, rx: &Receiver<Cmd>) {
    let mut backend = match backend() {
        Ok(b) => b,
        Err(e) => {
            log::warn!("overlay unavailable: {e:#}");
            // Keep draining so senders never notice.
            while rx.recv().is_ok() {}
            return;
        }
    };
    let mut hide_at: Option<Instant> = None;
    loop {
        match rx.recv_timeout(TICK) {
            Ok(Cmd::Show { monitor, panel, hide_after }) => {
                hide_at = hide_after.map(|d| Instant::now() + d);
                if let Some(pixels) = render::render(&panel, fonts)
                    && let Err(e) = backend.show(monitor.as_deref(), &pixels)
                {
                    log::warn!("overlay: {e:#}");
                }
            }
            Ok(Cmd::Hide) => {
                hide_at = None;
                backend.hide();
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if hide_at.is_some_and(|t| Instant::now() >= t) {
            hide_at = None;
            backend.hide();
        }
        if let Err(e) = backend.pump() {
            log::warn!("overlay stopped: {e:#}");
            while rx.recv().is_ok() {}
            return;
        }
    }
    backend.hide();
}

/// Top-left corner for a `width`-wide panel at the top-right of `monitor`
/// (desktop coordinates).
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
fn corner(monitor: &crate::Monitor, width: u32) -> (i32, i32) {
    (monitor.x + monitor.width as i32 - width as i32 - MARGIN, monitor.y + MARGIN)
}
