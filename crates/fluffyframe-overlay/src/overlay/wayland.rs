//! Wayland: a layer-shell surface on the overlay layer, with an empty input
//! region (clicks go through) and no keyboard focus, placed by its margins
//! from the output's top-right corner.
//!
//! In edit mode the surface covers the whole output instead (transparent, only
//! the panel takes the pointer) and the panel is drawn where it is dragged.
//! The surface itself never moves: pointer positions are relative to the
//! surface, and a surface moving under the pointer (late, or animated by the
//! compositor) would make a drag overshoot and run away.

use anyhow::{Context, Result};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
    Shape, WpCursorShapeDeviceV1,
};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::pointer::cursor_shape::CursorShapeManager;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry, delegate_seat, delegate_shm,
    registry_handlers,
};
use tiny_skia::Pixmap;
use wayland_client::backend::WaylandError;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output::WlOutput, wl_pointer::WlPointer, wl_seat::WlSeat, wl_shm, wl_surface::WlSurface};
use wayland_client::{Connection, EventQueue, QueueHandle};

use super::{Backend, Pointer, render::to_bgra};
use crate::monitors::wayland::monitor_of;

/// The panel to draw: its pixels (B, G, R, A premultiplied) and size.
struct PanelPixels {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

struct Shown {
    surface: LayerSurface,
    /// The output it was placed on (None: the compositor's choice).
    output: Option<String>,
    /// Edit mode: the surface covers the output and the panel is drawn in it.
    full: bool,
    /// The surface's size: the panel's, or the output's once configured.
    size: (u32, u32),
    /// The panel's top-right corner's distance from the output's.
    offset: (i32, i32),
    configured: bool,
    panel: PanelPixels,
    /// New pixels or position waiting to be drawn.
    dirty: bool,
}

impl Shown {
    /// The panel's top-left corner in the surface.
    fn panel_origin(&self) -> (i32, i32) {
        if self.full { (self.size.0 as i32 - self.offset.0 - self.panel.width as i32, self.offset.1) } else { (0, 0) }
    }
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: Option<SlotPool>,
    shown: Option<Shown>,
    seat: SeatState,
    /// The pointer, with its cursor-shape device when the compositor has one.
    pointer: Option<(WlPointer, Option<WpCursorShapeDeviceV1>)>,
    cursor_shapes: Option<CursorShapeManager>,
    /// Edit mode: the panel takes the pointer.
    interactive: bool,
    /// Pointer input on the panel since the last pump.
    input: Vec<Pointer>,
}

pub struct LayerOverlay {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
}

impl LayerOverlay {
    pub fn new() -> Result<Self> {
        let conn = Connection::connect_to_env().context("connecting to the Wayland compositor")?;
        let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
        let qh = queue.handle();
        let mut state = State {
            registry: RegistryState::new(&globals),
            outputs: OutputState::new(&globals, &qh),
            compositor: CompositorState::bind(&globals, &qh).context("no wl_compositor")?,
            layer_shell: LayerShell::bind(&globals, &qh).context("the compositor has no layer-shell")?,
            shm: Shm::bind(&globals, &qh).context("no wl_shm")?,
            pool: None,
            shown: None,
            seat: SeatState::new(&globals, &qh),
            pointer: None,
            cursor_shapes: CursorShapeManager::bind(&globals, &qh).ok(),
            interactive: false,
            input: Vec::new(),
        };
        queue.roundtrip(&mut state)?;
        queue.roundtrip(&mut state)?;
        Ok(Self { conn, queue, state })
    }
}

impl State {
    /// Draws the panel once the surface is configured: as the whole surface,
    /// or (edit mode) into a transparent output-sized one, where only the
    /// panel takes the pointer.
    fn draw(&mut self) -> Result<()> {
        let Some(shown) = self.shown.as_mut().filter(|s| s.configured && s.dirty) else { return Ok(()) };
        shown.dirty = false;
        let (w, h) = shown.size;
        if w == 0 || h == 0 {
            return Ok(());
        }
        let len = w as usize * h as usize * 4;
        let pool = match &mut self.pool {
            Some(pool) => pool,
            pool => pool.insert(SlotPool::new(len, &self.shm)?),
        };
        let (buffer, canvas) = pool.create_buffer(w as i32, h as i32, w as i32 * 4, wl_shm::Format::Argb8888)?;
        let (ox, oy) = shown.panel_origin();
        let panel = &shown.panel;
        if shown.full {
            canvas[..len].fill(0);
            blit(canvas, (w, h), panel, (ox, oy));
            let region = Region::new(&self.compositor)?;
            region.add(ox, oy, panel.width as i32, panel.height as i32);
            shown.surface.wl_surface().set_input_region(Some(region.wl_region()));
        } else {
            canvas[..panel.data.len().min(len)].copy_from_slice(&panel.data[..panel.data.len().min(len)]);
        }
        let surface = shown.surface.wl_surface();
        surface.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surface)?;
        shown.surface.commit();
        Ok(())
    }
}

/// Copies `panel` into a `size` canvas (B, G, R, A rows) at `at`, clipped.
fn blit(canvas: &mut [u8], (w, h): (u32, u32), panel: &PanelPixels, (x, y): (i32, i32)) {
    let row = panel.width as usize * 4;
    for py in 0..panel.height as i32 {
        let cy = y + py;
        if cy < 0 || cy >= h as i32 {
            continue;
        }
        // The part of the row inside the canvas.
        let (from, to) = ((-x).max(0), (w as i32 - x).min(panel.width as i32));
        if from >= to {
            continue;
        }
        let src = py as usize * row + from as usize * 4;
        let dst = (cy as usize * w as usize + (x + from) as usize) * 4;
        let n = (to - from) as usize * 4;
        canvas[dst..dst + n].copy_from_slice(&panel.data[src..src + n]);
    }
}

impl LayerOverlay {
    /// `result`, with the compositor's reason when it closed the connection
    /// over a protocol error (the I/O error alone says only "broken pipe").
    fn explain<T>(&self, result: Result<T>) -> Result<T> {
        result.map_err(|e| match self.conn.protocol_error() {
            Some(p) => anyhow::anyhow!("{e:#}: protocol error {} on {}@{}: {}", p.code, p.object_interface, p.object_id, p.message),
            None => e,
        })
    }
}

impl Backend for LayerOverlay {
    fn show(&mut self, monitor: Option<&str>, pixels: &Pixmap, right: i32, top: i32) -> Result<()> {
        let result = self.show_inner(monitor, pixels, right, top);
        self.explain(result)
    }

    fn hide(&mut self) {
        self.state.shown = None;
        let _ = self.conn.flush();
    }

    fn set_interactive(&mut self, on: bool) -> Result<()> {
        // The next show makes the surface for the new mode.
        self.state.interactive = on;
        Ok(())
    }

    fn pump(&mut self) -> Result<Vec<Pointer>> {
        let result = self.pump_inner();
        self.explain(result)
    }
}

impl LayerOverlay {
    fn show_inner(&mut self, monitor: Option<&str>, pixels: &Pixmap, right: i32, top: i32) -> Result<()> {
        let qh = self.queue.handle();
        let state = &mut self.state;
        let outputs: Vec<(WlOutput, String)> =
            state.outputs.outputs().filter_map(|o| state.outputs.info(&o).map(|i| (o, monitor_of(&i).name))).collect();
        // No monitor named (or it's gone): the compositor's choice, usually
        // the focused output.
        let output = monitor.and_then(|m| outputs.iter().find(|(_, n)| n == m));
        let name = output.map(|(_, n)| n.clone());
        let panel = PanelPixels { data: to_bgra(pixels), width: pixels.width(), height: pixels.height() };
        let full = state.interactive;

        if state.shown.as_ref().is_some_and(|s| s.output != name || s.full != full) {
            state.shown = None;
        }
        match &mut state.shown {
            Some(shown) => {
                let resized = !full && (shown.panel.width, shown.panel.height) != (panel.width, panel.height);
                let moved = !full && shown.offset != (right, top);
                shown.panel = panel;
                shown.offset = (right, top);
                shown.dirty = true;
                if moved {
                    shown.surface.set_margin(top, right, 0, 0);
                }
                if resized {
                    // Drawn once the compositor configures the new size.
                    shown.size = (shown.panel.width, shown.panel.height);
                    shown.configured = false;
                    shown.surface.set_size(shown.size.0, shown.size.1);
                }
                // One commit, so a new size and margin land together.
                if moved || resized {
                    shown.surface.commit();
                }
            }
            None => {
                let surface = state.compositor.create_surface(&qh);
                let layer = state.layer_shell.create_layer_surface(
                    &qh,
                    surface,
                    Layer::Overlay,
                    Some("fluffyframe-overlay"),
                    output.map(|(o, _)| o),
                );
                // Margins from the output's edges, whatever bars reserve: the
                // same place in and out of edit mode.
                layer.set_exclusive_zone(-1);
                layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                let size = if full {
                    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
                    layer.set_size(0, 0);
                    (0, 0)
                } else {
                    layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
                    layer.set_margin(top, right, 0, 0);
                    layer.set_size(panel.width, panel.height);
                    // Clicks go through.
                    let empty = Region::new(&state.compositor)?;
                    layer.wl_surface().set_input_region(Some(empty.wl_region()));
                    (panel.width, panel.height)
                };
                layer.commit();
                state.shown =
                    Some(Shown { surface: layer, output: name, full, size, offset: (right, top), configured: false, panel, dirty: true });
            }
        }
        state.draw()?;
        flush(&self.queue)?;
        Ok(())
    }

    fn pump_inner(&mut self) -> Result<Vec<Pointer>> {
        self.queue.dispatch_pending(&mut self.state)?;
        flush(&self.queue)?;
        if let Some(guard) = self.queue.prepare_read() {
            let fd = guard.connection_fd();
            let mut fds = [rustix::event::PollFd::new(&fd, rustix::event::PollFlags::IN)];
            let zero = rustix::event::Timespec { tv_sec: 0, tv_nsec: 0 };
            let ready = rustix::event::poll(&mut fds, Some(&zero)).unwrap_or(0) > 0;
            if ready {
                match guard.read() {
                    Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                    other => {
                        other?;
                    }
                }
            }
        }
        self.queue.dispatch_pending(&mut self.state)?;
        flush(&self.queue)?;
        Ok(std::mem::take(&mut self.state.input))
    }
}

impl PointerHandler for State {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlPointer, events: &[PointerEvent]) {
        let Some(shown) = self.shown.as_ref().filter(|s| s.full) else { return };
        let surface = shown.surface.wl_surface().clone();
        // Surface positions to panel ones: the surface stays put, so this is
        // exact even mid-drag.
        let (ox, oy) = shown.panel_origin();
        for event in events.iter().filter(|e| e.surface == surface) {
            let (x, y) = (event.position.0 - f64::from(ox), event.position.1 - f64::from(oy));
            match event.kind {
                PointerEventKind::Enter { serial } => {
                    if let Some((_, Some(device))) = &self.pointer {
                        device.set_shape(serial, Shape::Move);
                    }
                }
                PointerEventKind::Press { button: BTN_LEFT, .. } => self.input.push(Pointer::Press(x, y)),
                PointerEventKind::Motion { .. } => self.input.push(Pointer::Motion(x, y)),
                PointerEventKind::Release { button: BTN_LEFT, .. } => self.input.push(Pointer::Release),
                _ => {}
            }
        }
    }
}

/// The left mouse button's evdev code.
const BTN_LEFT: u32 = 0x110;

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: WlSeat, capability: Capability) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat.get_pointer(qh, &seat) {
                Ok(pointer) => {
                    let device = self.cursor_shapes.as_ref().map(|m| m.get_shape_device(&pointer, qh));
                    self.pointer = Some((pointer, device));
                }
                Err(e) => log::warn!("overlay pointer: {e}"),
            }
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat, capability: Capability) {
        if capability == Capability::Pointer
            && let Some((pointer, device)) = self.pointer.take()
        {
            if let Some(device) = device {
                device.destroy();
            }
            pointer.release();
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

/// Sends queued requests; a full socket just means "later".
fn flush(queue: &EventQueue<State>) -> Result<()> {
    match queue.flush() {
        Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        other => Ok(other?),
    }
}

impl LayerShellHandler for State {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.shown = None;
    }

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, configure: LayerSurfaceConfigure, _: u32) {
        if let Some(shown) = self.shown.as_mut().filter(|s| &s.surface == layer) {
            // Edit mode's surface is as big as the compositor makes it: the output.
            if shown.full && configure.new_size.0 > 0 && configure.new_size.1 > 0 {
                shown.size = configure.new_size;
            }
            shown.configured = true;
            shown.dirty = true;
            if let Err(e) = self.draw() {
                log::warn!("overlay draw: {e:#}");
            }
        }
    }
}

impl CompositorHandler for State {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: wayland_client::protocol::wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {}
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: &WlOutput) {}
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
}

impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(State);
delegate_layer!(State);
delegate_output!(State);
delegate_pointer!(State);
delegate_seat!(State);
delegate_registry!(State);
delegate_shm!(State);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blits_the_panel_clipped_to_the_canvas() {
        let panel = PanelPixels { data: vec![7; 2 * 2 * 4], width: 2, height: 2 };
        let mut canvas = vec![0u8; 3 * 3 * 4];
        // Half off the right edge: one column lands.
        blit(&mut canvas, (3, 3), &panel, (2, 1));
        let px = |x: usize, y: usize| canvas[(y * 3 + x) * 4];
        assert_eq!((px(2, 1), px(2, 2), px(1, 1), px(2, 0)), (7, 7, 0, 0));
        // Fully off: nothing, and no panic.
        blit(&mut canvas, (3, 3), &panel, (-5, 9));
    }
}
