//! A click-through panel drawn over the game, near the top-right corner of a
//! monitor. It takes no focus and no input, and window managers don't place
//! it, so tiling ones leave the layout alone:
//!
//! - Wayland: a wlr-layer-shell surface on the overlay layer (Hyprland, Sway,
//!   KDE, river, niri, COSMIC…); above fullscreen windows.
//! - X11, and Wayland desktops without layer-shell (GNOME, through XWayland):
//!   an override-redirect window with an empty input shape.
//! - Windows: a layered, topmost, transparent tool window.
//!
//! In edit mode ([`Overlay::set_editing`]) it takes the mouse instead: drag it
//! to move it, drag its bottom-right grip to resize it. The [`Layout`] is
//! reported back when a drag ends, for saving.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
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

/// How often the platform's events are handled between commands.
const TICK: Duration = Duration::from_millis(16);
/// Sizes the panel can be scaled to.
pub const MIN_SCALE: f32 = 0.6;
pub const MAX_SCALE: f32 = 2.5;

/// What the overlay shows: a title and lines under it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Panel {
    pub title: String,
    pub lines: Vec<String>,
}

/// Where the panel sits and how big it is: its top-right corner's distance
/// from the monitor's top-right corner (logical pixels), so a wider panel
/// grows leftwards and stays on screen, and its scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub right: i32,
    pub top: i32,
    pub scale: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self { right: 32, top: 32, scale: 1.0 }
    }
}

enum Cmd {
    Show { monitor: Option<String>, panel: Panel, hide_after: Option<Duration> },
    Hide,
    SetLayout(Layout),
    /// Edit mode with a panel to place when nothing else is shown; `None` ends it.
    Edit(Option<Panel>),
}

/// Handle to the overlay thread; dropping it closes the overlay.
pub struct Overlay {
    tx: Sender<Cmd>,
    editing: Arc<AtomicBool>,
}

impl Overlay {
    /// Starts the overlay thread. `fonts` are tried in order per character;
    /// `on_layout` gets the layout after the user moves or resizes the panel.
    pub fn start(fonts: Vec<&'static [u8]>, layout: Layout, on_layout: impl Fn(Layout) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel();
        let editing = Arc::new(AtomicBool::new(false));
        let thread_editing = editing.clone();
        let spawned = std::thread::Builder::new()
            .name("overlay".into())
            .spawn(move || run(&Fonts::new(&fonts), &rx, layout, &on_layout, &thread_editing));
        if let Err(e) = spawned {
            log::warn!("overlay thread: {e}");
        }
        Self { tx, editing }
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

    pub fn set_layout(&self, layout: Layout) {
        let _ = self.tx.send(Cmd::SetLayout(layout));
    }

    /// Unlocks moving and resizing (`Some`: the panel to place when nothing is
    /// shown), or locks the panel again (`None`).
    pub fn set_editing(&self, sample: Option<Panel>) {
        self.editing.store(sample.is_some(), Ordering::Relaxed);
        let _ = self.tx.send(Cmd::Edit(sample));
    }

    pub fn editing(&self) -> bool {
        self.editing.load(Ordering::Relaxed)
    }
}

/// Pointer input on the panel in edit mode. `at` is in a frame that doesn't
/// move with the panel (the screen, or a surface covering the output): a drag
/// is the pointer's travel since the press in that frame, so events arriving
/// several at a time can't add up to more than the pointer moved.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pointer {
    /// `local`: where on the panel (for the resize grip).
    Press { local: (f64, f64), at: (f64, f64) },
    Motion { at: (f64, f64) },
    Release,
}

/// A platform's way of putting pixels on screen.
trait Backend {
    /// Shows `pixels` (premultiplied RGBA) on `monitor`, its top-right corner
    /// `right` and `top` pixels in from the monitor's.
    fn show(&mut self, monitor: Option<&str>, pixels: &Pixmap, right: i32, top: i32) -> anyhow::Result<()>;
    fn hide(&mut self);
    /// Takes the mouse (edit mode), or lets clicks through.
    fn set_interactive(&mut self, on: bool) -> anyhow::Result<()>;
    /// Handles pending platform events (redraws, configure…), returning the
    /// pointer input on the panel.
    fn pump(&mut self) -> anyhow::Result<Vec<Pointer>>;
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

/// A drag in progress, from where the pointer went down.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Drag {
    /// From the press: where the pointer was and the panel's place.
    Move { at: (f64, f64), right: i32, top: i32 },
    Resize { x: f64, width: f64, scale: f32, right: i32 },
}

/// The overlay thread's state: what to show, where, and edit mode.
struct Shown<'a> {
    fonts: &'a Fonts,
    layout: Layout,
    monitor: Option<String>,
    panel: Option<Panel>,
    hide_at: Option<Instant>,
    /// The panel placed in edit mode when nothing else is shown.
    sample: Option<Panel>,
    drag: Option<Drag>,
    /// The size last shown.
    size: (u32, u32),
}

impl Shown<'_> {
    fn editing(&self) -> bool {
        self.sample.is_some()
    }

    /// What should be on screen now, drawn at the current scale.
    fn pixels(&self) -> Option<Pixmap> {
        let panel = self.panel.as_ref().or(self.sample.as_ref())?;
        render::render(panel, self.fonts, self.layout.scale, self.editing())
    }

    /// Draws what should be on screen now, or hides the overlay.
    fn redraw(&mut self, backend: &mut dyn Backend) {
        match self.pixels() {
            Some(pixels) => self.present(backend, &pixels),
            None => backend.hide(),
        }
    }

    fn present(&mut self, backend: &mut dyn Backend, pixels: &Pixmap) {
        self.size = (pixels.width(), pixels.height());
        if let Err(e) = backend.show(self.monitor.as_deref(), pixels, self.layout.right, self.layout.top) {
            log::warn!("overlay: {e:#}");
        }
    }

    /// Applies pointer input; true when a drag ended (the layout is final).
    fn pointer(&mut self, input: Pointer, backend: &mut dyn Backend) -> bool {
        match (input, self.drag) {
            (Pointer::Press { local: (x, y), at }, _) => {
                let width = f64::from(self.size.0);
                let grip = f64::from(render::grip(self.layout.scale));
                self.drag = Some(if x >= width - grip && y >= f64::from(self.size.1) - grip {
                    Drag::Resize { x: at.0, width, scale: self.layout.scale, right: self.layout.right }
                } else {
                    Drag::Move { at, right: self.layout.right, top: self.layout.top }
                });
            }
            (Pointer::Motion { at: (x, y) }, Some(Drag::Move { at: (x0, y0), right, top })) => {
                // Where the panel was at the press, plus the pointer's travel.
                let place = (right - (x - x0).round() as i32, top + (y - y0).round() as i32);
                if place != (self.layout.right, self.layout.top) {
                    (self.layout.right, self.layout.top) = place;
                    self.redraw(backend);
                }
            }
            (Pointer::Motion { at: (x, _) }, Some(Drag::Resize { x: x0, width, scale, right })) => {
                // The grip follows the pointer: the width grows by as much as
                // it moved, the top-left corner stays put.
                let wanted = (width + x - x0).max(1.0);
                let next = (scale * (wanted / width) as f32).clamp(MIN_SCALE, MAX_SCALE);
                if (next - self.layout.scale).abs() > 0.005 {
                    self.layout.scale = next;
                    if let Some(pixels) = self.pixels() {
                        self.layout.right = right - (f64::from(pixels.width()) - width).round() as i32;
                        self.present(backend, &pixels);
                    }
                }
            }
            (Pointer::Release, Some(_)) => {
                self.drag = None;
                return true;
            }
            _ => {}
        }
        false
    }
}

fn run(fonts: &Fonts, rx: &Receiver<Cmd>, layout: Layout, on_layout: &dyn Fn(Layout), editing: &AtomicBool) {
    let mut backend = match backend() {
        Ok(b) => b,
        Err(e) => {
            log::warn!("overlay unavailable: {e:#}");
            editing.store(false, Ordering::Relaxed);
            // Keep draining so senders never notice.
            while rx.recv().is_ok() {}
            return;
        }
    };
    let mut shown = Shown { fonts, layout, monitor: None, panel: None, hide_at: None, sample: None, drag: None, size: (0, 0) };
    loop {
        match rx.recv_timeout(TICK) {
            Ok(Cmd::Show { monitor, panel, hide_after }) => {
                shown.hide_at = hide_after.map(|d| Instant::now() + d);
                shown.monitor = monitor;
                shown.panel = Some(panel);
                shown.redraw(backend.as_mut());
            }
            Ok(Cmd::Hide) => {
                shown.hide_at = None;
                shown.panel = None;
                shown.redraw(backend.as_mut());
            }
            Ok(Cmd::SetLayout(layout)) => {
                shown.layout = layout;
                shown.redraw(backend.as_mut());
            }
            Ok(Cmd::Edit(sample)) => {
                let ended = shown.sample.is_some() && sample.is_none();
                shown.sample = sample;
                shown.drag = None;
                if let Err(e) = backend.set_interactive(shown.editing()) {
                    log::warn!("overlay edit mode: {e:#}");
                }
                shown.redraw(backend.as_mut());
                if ended {
                    on_layout(shown.layout);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if shown.hide_at.is_some_and(|t| Instant::now() >= t) {
            shown.hide_at = None;
            shown.panel = None;
            shown.redraw(backend.as_mut());
        }
        match backend.pump() {
            Ok(inputs) => {
                // Only the latest position of a run of motion is drawn.
                let last_motion = |i: usize| matches!(inputs.get(i + 1), Some(Pointer::Motion { .. }));
                let inputs: Vec<Pointer> =
                    inputs.iter().enumerate().filter(|(i, p)| !(matches!(p, Pointer::Motion { .. }) && last_motion(*i))).map(|(_, p)| *p).collect();
                for input in inputs {
                    if shown.editing() && shown.pointer(input, backend.as_mut()) {
                        on_layout(shown.layout);
                    }
                }
            }
            Err(e) => {
                log::warn!("overlay stopped: {e:#}");
                editing.store(false, Ordering::Relaxed);
                while rx.recv().is_ok() {}
                return;
            }
        }
    }
    backend.hide();
}

/// Top-left corner, in desktop coordinates, of a `width`-wide panel whose
/// top-right corner is `right` and `top` in from `monitor`'s.
#[cfg_attr(not(any(target_os = "linux", windows)), allow(dead_code))]
fn corner(monitor: &crate::Monitor, width: u32, right: i32, top: i32) -> (i32, i32) {
    (monitor.x + monitor.width as i32 - width as i32 - right, monitor.y + top)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records what was shown where, like a screen would.
    #[derive(Default)]
    struct Screen {
        shown: Option<(u32, i32, i32)>,
    }

    impl Backend for Screen {
        fn show(&mut self, _: Option<&str>, pixels: &Pixmap, right: i32, top: i32) -> anyhow::Result<()> {
            self.shown = Some((pixels.width(), right, top));
            Ok(())
        }
        fn hide(&mut self) {
            self.shown = None;
        }
        fn set_interactive(&mut self, _: bool) -> anyhow::Result<()> {
            Ok(())
        }
        fn pump(&mut self) -> anyhow::Result<Vec<Pointer>> {
            Ok(Vec::new())
        }
    }

    fn editing(fonts: &Fonts) -> Shown<'_> {
        let sample = Panel { title: "Riven".into(), lines: vec!["a line of text".into()] };
        Shown { fonts, layout: Layout::default(), monitor: None, panel: None, hide_at: None, sample: Some(sample), drag: None, size: (0, 0) }
    }

    #[test]
    fn dragging_moves_the_panel() {
        let fonts = Fonts::new(&[]);
        let (mut screen, mut shown) = (Screen::default(), editing(&fonts));
        shown.redraw(&mut screen);
        assert!(!shown.pointer(Pointer::Press { local: (20.0, 10.0), at: (500.0, 300.0) }, &mut screen));
        // 30 px right and 5 down: 30 px closer to the right edge.
        shown.pointer(Pointer::Motion { at: (530.0, 305.0) }, &mut screen);
        assert_eq!((shown.layout.right, shown.layout.top), (2, 37));
        assert_eq!(screen.shown.map(|s| (s.1, s.2)), Some((2, 37)));
        // Events that arrive together don't add up: each is the travel since the press.
        shown.pointer(Pointer::Motion { at: (540.0, 305.0) }, &mut screen);
        shown.pointer(Pointer::Motion { at: (550.0, 305.0) }, &mut screen);
        assert_eq!((shown.layout.right, shown.layout.top), (-18, 37));
        assert!(shown.pointer(Pointer::Release, &mut screen));
    }

    #[test]
    fn dragging_the_grip_resizes_and_keeps_the_left_edge() {
        let fonts = Fonts::new(&[]);
        let (mut screen, mut shown) = (Screen::default(), editing(&fonts));
        shown.redraw(&mut screen);
        let (w, h) = (f64::from(shown.size.0), f64::from(shown.size.1));
        let left = -(w as i32) - shown.layout.right;
        shown.pointer(Pointer::Press { local: (w - 2.0, h - 2.0), at: (w - 2.0, h - 2.0) }, &mut screen);
        assert!(matches!(shown.drag, Some(Drag::Resize { .. })));
        shown.pointer(Pointer::Motion { at: (w * 1.5 - 2.0, h) }, &mut screen);
        assert!((shown.layout.scale - 1.5).abs() < 0.05, "{}", shown.layout.scale);
        // The left edge (monitor width − panel width − right) didn't move.
        assert_eq!(-(shown.size.0 as i32) - shown.layout.right, left);
        // Never past the limits.
        shown.pointer(Pointer::Motion { at: (w * 10.0, h) }, &mut screen);
        assert_eq!(shown.layout.scale, MAX_SCALE);
    }

    #[test]
    fn locked_panels_ignore_the_pointer_and_hide_when_empty() {
        let fonts = Fonts::new(&[]);
        let (mut screen, mut shown) = (Screen::default(), editing(&fonts));
        shown.redraw(&mut screen);
        assert!(screen.shown.is_some());
        shown.sample = None;
        shown.redraw(&mut screen);
        assert!(screen.shown.is_none(), "nothing to show once edit mode ends");
    }
}
