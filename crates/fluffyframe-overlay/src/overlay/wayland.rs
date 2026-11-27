//! Wayland: a layer-shell surface on the overlay layer, with an empty input
//! region (clicks go through) and no keyboard focus.

use anyhow::{Context, Result};
use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState, Region};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm, registry_handlers,
};
use tiny_skia::Pixmap;
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::{wl_output::WlOutput, wl_shm, wl_surface::WlSurface};
use wayland_client::backend::WaylandError;
use wayland_client::{Connection, EventQueue, QueueHandle};

use super::{Backend, MARGIN, render::to_bgra};
use crate::monitors::wayland::monitor_of;

struct Shown {
    surface: LayerSurface,
    /// The output it was placed on (None: the compositor's choice).
    output: Option<String>,
    size: (u32, u32),
    configured: bool,
    /// Pixels waiting for a configure (B, G, R, A premultiplied).
    pending: Option<Vec<u8>>,
}

struct State {
    registry: RegistryState,
    outputs: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: Option<SlotPool>,
    shown: Option<Shown>,
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
        };
        queue.roundtrip(&mut state)?;
        queue.roundtrip(&mut state)?;
        Ok(Self { conn, queue, state })
    }
}

impl State {
    /// Attaches the pending pixels once the surface is configured.
    fn draw(&mut self) -> Result<()> {
        let Some(shown) = self.shown.as_mut().filter(|s| s.configured) else { return Ok(()) };
        let Some(pixels) = shown.pending.take() else { return Ok(()) };
        let (w, h) = shown.size;
        let pool = match &mut self.pool {
            Some(pool) => pool,
            pool => pool.insert(SlotPool::new(pixels.len(), &self.shm)?),
        };
        let (buffer, canvas) = pool.create_buffer(w as i32, h as i32, w as i32 * 4, wl_shm::Format::Argb8888)?;
        canvas[..pixels.len()].copy_from_slice(&pixels);
        let surface = shown.surface.wl_surface();
        surface.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surface)?;
        shown.surface.commit();
        Ok(())
    }
}

impl Backend for LayerOverlay {
    fn show(&mut self, monitor: Option<&str>, pixels: &Pixmap) -> Result<()> {
        let qh = self.queue.handle();
        let state = &mut self.state;
        let outputs: Vec<(WlOutput, String)> =
            state.outputs.outputs().filter_map(|o| state.outputs.info(&o).map(|i| (o, monitor_of(&i).name))).collect();
        let output = monitor.and_then(|m| outputs.iter().find(|(_, n)| n == m)).or(outputs.first());
        let name = output.map(|(_, n)| n.clone());
        let size = (pixels.width(), pixels.height());

        if state.shown.as_ref().is_some_and(|s| s.output != name) {
            state.shown = None;
        }
        match &mut state.shown {
            Some(shown) => {
                shown.pending = Some(to_bgra(pixels));
                if shown.size != size {
                    // Drawn once the compositor configures the new size.
                    shown.size = size;
                    shown.configured = false;
                    shown.surface.set_size(size.0, size.1);
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
                layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
                layer.set_margin(MARGIN, MARGIN, 0, 0);
                layer.set_size(size.0, size.1);
                layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                let empty = Region::new(&state.compositor)?;
                layer.wl_surface().set_input_region(Some(empty.wl_region()));
                layer.commit();
                state.shown = Some(Shown { surface: layer, output: name, size, configured: false, pending: Some(to_bgra(pixels)) });
            }
        }
        state.draw()?;
        flush(&self.queue)?;
        Ok(())
    }

    fn hide(&mut self) {
        self.state.shown = None;
        let _ = self.conn.flush();
    }

    fn pump(&mut self) -> Result<()> {
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
        Ok(())
    }
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

    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, _: LayerSurfaceConfigure, _: u32) {
        if let Some(shown) = self.shown.as_mut().filter(|s| &s.surface == layer) {
            shown.configured = true;
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
    registry_handlers![OutputState];
}

delegate_compositor!(State);
delegate_layer!(State);
delegate_output!(State);
delegate_registry!(State);
delegate_shm!(State);
