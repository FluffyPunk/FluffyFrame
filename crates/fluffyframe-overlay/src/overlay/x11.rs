//! X11: an override-redirect window (window managers don't manage or tile it)
//! with a 32-bit visual for transparency and an empty input shape. In edit
//! mode the input shape is the whole window and button events come through.

use anyhow::{Context, Result};
use tiny_skia::Pixmap;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{
    AtomEnum, ClipOrdering, ColormapAlloc, ConfigureWindowAux, ConnectionExt as _, CreateGCAux, CreateWindowAux,
    EventMask, ImageFormat, PropMode, StackMode, VisualClass, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::{Backend, Pointer, corner, render::to_bgra};

pub struct X11Overlay {
    conn: RustConnection,
    window: u32,
    gc: u32,
    depth: u8,
    mapped: bool,
    /// Where the window was last placed (root coordinates): pointer input is
    /// measured from it, not from where the server has it yet, so a drag
    /// doesn't run ahead of the pointer.
    position: (i32, i32),
    shape: bool,
    /// What is shown (B, G, R, A premultiplied), for redraws on expose.
    shown: Option<(Vec<u8>, u16, u16)>,
}

impl X11Overlay {
    pub fn new() -> Result<Self> {
        let (conn, screen) = crate::monitors::x11::connect()?;
        let root = conn.setup().roots[screen].clone();
        // A 32-bit TrueColor visual gives per-pixel alpha under a compositor;
        // without one the panel is drawn over black.
        let argb = root
            .allowed_depths
            .iter()
            .filter(|d| d.depth == 32)
            .flat_map(|d| d.visuals.iter())
            .find(|v| v.class == VisualClass::TRUE_COLOR)
            .map(|v| v.visual_id);
        let (depth, visual) = match argb {
            Some(v) => (32, v),
            None => (root.root_depth, root.root_visual),
        };
        let colormap = conn.generate_id()?;
        conn.create_colormap(ColormapAlloc::NONE, colormap, root.root, visual)?;
        let window = conn.generate_id()?;
        let aux = CreateWindowAux::new()
            .override_redirect(1)
            .background_pixel(0)
            .border_pixel(0)
            .colormap(colormap)
            .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::BUTTON1_MOTION);
        conn.create_window(depth, window, root.root, 0, 0, 1, 1, 0, WindowClass::INPUT_OUTPUT, visual, &aux)?;
        conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_NAME, AtomEnum::STRING, b"FluffyFrame overlay")?;
        conn.change_property8(PropMode::REPLACE, window, AtomEnum::WM_CLASS, AtomEnum::STRING, b"fluffyframe-overlay\0FluffyFrame\0")?;
        let shape = conn.extension_information(shape::X11_EXTENSION_NAME)?.is_some();
        if !shape {
            log::warn!("X server has no SHAPE extension: the overlay will catch clicks");
        }
        let gc = conn.generate_id()?;
        conn.create_gc(gc, window, &CreateGCAux::new())?;
        let mut overlay = Self { conn, window, gc, depth, mapped: false, position: (0, 0), shape, shown: None };
        overlay.set_interactive(false)?;
        Ok(overlay)
    }

    fn paint(&self) -> Result<()> {
        let Some((data, w, h)) = &self.shown else { return Ok(()) };
        // Split the image so each request stays under the server's limit.
        let row = usize::from(*w) * 4;
        let rows_per = ((self.conn.maximum_request_bytes() - 64) / row).clamp(1, usize::from(*h));
        for (i, chunk) in data.chunks(row * rows_per).enumerate() {
            let y = (i * rows_per) as i16;
            let rows = (chunk.len() / row) as u16;
            self.conn.put_image(ImageFormat::Z_PIXMAP, self.window, self.gc, *w, rows, 0, y, 0, self.depth, chunk)?;
        }
        self.conn.flush()?;
        Ok(())
    }
}

impl Backend for X11Overlay {
    fn show(&mut self, monitor: Option<&str>, pixels: &Pixmap, right: i32, top: i32) -> Result<()> {
        let monitors = crate::monitors::x11::monitors()?;
        let target = crate::pick(&monitors, monitor).context("no monitor")?;
        let (x, y) = corner(target, pixels.width(), right, top);
        self.position = (x, y);
        let (w, h) = (pixels.width() as u16, pixels.height() as u16);
        let aux = ConfigureWindowAux::new().x(x).y(y).width(u32::from(w)).height(u32::from(h)).stack_mode(StackMode::ABOVE);
        self.conn.configure_window(self.window, &aux)?;
        if !self.mapped {
            self.conn.map_window(self.window)?;
            self.mapped = true;
        }
        self.shown = Some((to_bgra(pixels), w, h));
        self.paint()
    }

    fn hide(&mut self) {
        if self.mapped {
            let _ = self.conn.unmap_window(self.window);
            let _ = self.conn.flush();
            self.mapped = false;
        }
        self.shown = None;
    }

    fn set_interactive(&mut self, on: bool) -> Result<()> {
        if self.shape {
            if on {
                // No input shape: the window's own rectangle takes the pointer.
                self.conn.shape_mask(shape::SO::SET, shape::SK::INPUT, self.window, 0, 0, x11rb::NONE)?;
            } else {
                // Clicks pass through: an empty input shape.
                self.conn.shape_rectangles(shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED, self.window, 0, 0, &[])?;
            }
        }
        self.conn.flush()?;
        Ok(())
    }

    fn pump(&mut self) -> Result<Vec<Pointer>> {
        let mut expose = false;
        let mut input = Vec::new();
        // Panel pixels from root coordinates and where the window was put.
        let at = |x: i16, y: i16| (f64::from(i32::from(x) - self.position.0), f64::from(i32::from(y) - self.position.1));
        while let Some(event) = self.conn.poll_for_event()? {
            match event {
                Event::Expose(_) => expose = true,
                Event::ButtonPress(e) if e.detail == 1 => {
                    let (x, y) = at(e.root_x, e.root_y);
                    input.push(Pointer::Press(x, y));
                }
                Event::MotionNotify(e) => {
                    let (x, y) = at(e.root_x, e.root_y);
                    input.push(Pointer::Motion(x, y));
                }
                Event::ButtonRelease(e) if e.detail == 1 => input.push(Pointer::Release),
                _ => {}
            }
        }
        if expose {
            self.paint()?;
        }
        Ok(input)
    }
}
