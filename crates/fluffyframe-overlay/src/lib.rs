//! A click-through panel over the game ([`overlay`]), placed on a monitor
//! ([`monitors`]): Windows, X11, and Wayland compositors with or without
//! layer-shell. Drawn in software, so every platform shows the same pixels.

pub mod monitors;
pub mod overlay;

/// A monitor in desktop coordinates (logical pixels).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Monitor {
    /// The output's name (`DP-2`, `\\.\DISPLAY1`): what settings store.
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The monitor named `name`, or the first one when it is unknown or gone.
pub fn pick<'a>(monitors: &'a [Monitor], name: Option<&str>) -> Option<&'a Monitor> {
    name.and_then(|n| monitors.iter().find(|m| m.name == n)).or(monitors.first())
}
