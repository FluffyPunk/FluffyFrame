//! Windows: monitors through xcap.

use anyhow::Result;

use crate::Monitor;

pub fn monitors() -> Result<Vec<Monitor>> {
    let mut out = Vec::new();
    for m in xcap::Monitor::all()? {
        out.push(Monitor { name: m.name()?, x: m.x()?, y: m.y()?, width: m.width()?, height: m.height()? });
    }
    Ok(out)
}
