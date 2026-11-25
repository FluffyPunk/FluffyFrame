//! Per-user data locations (original: `services/userDataPath.ts`).
//!
//! `WFHELPER_DATA_DIR` overrides the base directory, for tests and portable installs.

use std::path::PathBuf;
use std::sync::OnceLock;

use super::app_meta::APP_ID;

static BASE: OnceLock<PathBuf> = OnceLock::new();

pub fn data_dir() -> &'static PathBuf {
    BASE.get_or_init(|| {
        let dir = std::env::var_os("WFHELPER_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                directories::ProjectDirs::from("com", "WFHelper", APP_ID)
                    .map(|d| d.data_dir().to_path_buf())
            })
            .unwrap_or_else(|| PathBuf::from(".wfhelper"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    })
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
    user_file("wfhelper.log")
}
