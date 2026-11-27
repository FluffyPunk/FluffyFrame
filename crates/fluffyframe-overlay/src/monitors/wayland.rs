//! Wayland: outputs, with their logical layout when the compositor reports it.

use anyhow::{Context, Result};
use smithay_client_toolkit::output::{OutputHandler, OutputInfo, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::{delegate_output, delegate_registry, registry_handlers};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::{Connection, QueueHandle};

use crate::Monitor;

struct State {
    registry: RegistryState,
    outputs: OutputState,
}

/// A monitor from an output's info: its logical position and size when the
/// compositor reports them (xdg-output), else its mode.
pub(crate) fn monitor_of(info: &OutputInfo) -> Monitor {
    let mode = info.modes.iter().find(|m| m.current).map_or((0, 0), |m| m.dimensions);
    let (x, y) = info.logical_position.unwrap_or(info.location);
    let (w, h) = info.logical_size.unwrap_or(mode);
    Monitor {
        name: info.name.clone().unwrap_or_else(|| format!("{} {}", info.make, info.model)),
        x,
        y,
        width: w.max(0) as u32,
        height: h.max(0) as u32,
    }
}

pub fn monitors() -> Result<Vec<Monitor>> {
    let conn = Connection::connect_to_env().context("connecting to the Wayland compositor")?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn).context("reading Wayland globals")?;
    let qh = queue.handle();
    let mut state = State { registry: RegistryState::new(&globals), outputs: OutputState::new(&globals, &qh) };
    // Output details (and xdg-output's logical layout) arrive over two trips.
    queue.roundtrip(&mut state)?;
    queue.roundtrip(&mut state)?;
    Ok(state.outputs.outputs().filter_map(|o| state.outputs.info(&o)).map(|i| monitor_of(&i)).collect())
}

impl OutputHandler for State {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlOutput) {}
}

impl ProvidesRegistryState for State {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}

delegate_output!(State);
delegate_registry!(State);
