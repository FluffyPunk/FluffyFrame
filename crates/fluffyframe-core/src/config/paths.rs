//! Per-user data locations (original: `services/userDataPath.ts`).
//!
//! `FLUFFYFRAME_DATA_DIR` overrides the base directory, for tests and portable installs.
//! Data from before the rename (the `wfhelper-rust` folder) is moved over on first launch.

use std::path::PathBuf;
use std::sync::OnceLock;

use super::app_meta::{self, APP_ID, APP_NAME};

static BASE: OnceLock<PathBuf> = OnceLock::new();

pub fn data_dir() -> &'static PathBuf {
    BASE.get_or_init(|| {
        let dir = app_meta::env_var("DATA_DIR")
            .map(PathBuf::from)
            .or_else(platform_dir)
            .unwrap_or_else(|| PathBuf::from(".fluffyframe"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    })
}

/// The platform data folder, taking over the pre-rename one when it is the only
/// one there. If the move fails, the old folder stays in use, so nothing is lost.
fn platform_dir() -> Option<PathBuf> {
    let new = directories::ProjectDirs::from("com", APP_NAME, APP_ID)?.data_dir().to_path_buf();
    let old = directories::ProjectDirs::from("com", "WFHelper", "wfhelper-rust").map(|d| d.data_dir().to_path_buf());
    match old {
        Some(old) if !new.exists() && old.is_dir() => {
            if let Some(parent) = new.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            Some(if std::fs::rename(&old, &new).is_ok() { new } else { old })
        }
        _ => Some(new),
    }
}

/// A file directly under the data directory.
pub fn user_file(name: &str) -> PathBuf {
    data_dir().join(name)
}

/// A file under `cache/`, creating the folder on demand.
pub fn cache_file(name: &str) -> PathBuf {
    let path = data_dir().join("cache").join(name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    path
}

pub fn settings_file() -> PathBuf {
    user_file("settings.json")
}
pub fn inventory_snapshot_file() -> PathBuf {
    user_file("inventory.json")
}
pub fn trades_file() -> PathBuf {
    user_file("trades.json")
}
pub fn stats_file() -> PathBuf {
    user_file("stats.json")
}
pub fn wfm_session_file() -> PathBuf {
    user_file("wfm-session.json")
}
pub fn log_file() -> PathBuf {
    user_file("fluffyframe.log")
}
