//! The display server and its monitors, for placing the overlay.
//!
//! - Wayland: outputs (with xdg-output's logical layout).
//! - X11: RandR monitors.
//! - Windows: the system's monitors.

use anyhow::{Result, bail};

use crate::Monitor;

#[cfg(target_os = "linux")]
pub(crate) mod wayland;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
pub(crate) mod x11;

/// The display server the app runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    Wayland,
    X11,
    Windows,
    Unsupported,
}

pub fn session() -> Session {
    if cfg!(windows) {
        Session::Windows
    } else if cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some() {
        Session::Wayland
    } else if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_some() {
        Session::X11
    } else {
        Session::Unsupported
    }
}

/// The monitors, in the display server's order.
pub fn monitors() -> Result<Vec<Monitor>> {
    match session() {
        #[cfg(target_os = "linux")]
        Session::Wayland => wayland::monitors(),
        #[cfg(target_os = "linux")]
        Session::X11 => x11::monitors(),
        #[cfg(windows)]
        Session::Windows => windows::monitors(),
        _ => bail!("no display server to list monitors from"),
    }
}
