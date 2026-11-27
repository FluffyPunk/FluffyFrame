//! X11: monitors from RandR.

use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use crate::Monitor;

pub(crate) fn connect() -> Result<(RustConnection, usize)> {
    x11rb::connect(None).context("connecting to the X server")
}

pub fn monitors() -> Result<Vec<Monitor>> {
    let (conn, screen) = connect()?;
    let root = &conn.setup().roots[screen];
    let randr = conn.randr_get_monitors(root.root, true).ok().and_then(|c| c.reply().ok());
    let mut out = Vec::new();
    for m in randr.map(|r| r.monitors).unwrap_or_default() {
        let name = conn
            .get_atom_name(m.name)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| String::from_utf8_lossy(&r.name).into_owned())
            .unwrap_or_else(|| format!("monitor {}", out.len() + 1));
        out.push(Monitor { name, x: m.x.into(), y: m.y.into(), width: m.width.into(), height: m.height.into() });
    }
    if out.is_empty() {
        let (width, height) = (root.width_in_pixels.into(), root.height_in_pixels.into());
        out.push(Monitor { name: "screen".into(), x: 0, y: 0, width, height });
    }
    Ok(out)
}
