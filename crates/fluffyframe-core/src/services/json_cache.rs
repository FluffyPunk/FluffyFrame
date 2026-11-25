//! Disk cache for downloaded payloads (original: `services/jsonCache.ts`).

use std::time::{Duration, SystemTime};

use crate::config::paths;

/// Cached text when present and younger than `max_age` (`None` max age = any age).
pub fn read(name: &str, max_age: Option<Duration>) -> Option<String> {
    let path = paths::cache_file(name);
    let meta = std::fs::metadata(&path).ok()?;
    if let Some(max_age) = max_age {
        let age = SystemTime::now()
            .duration_since(meta.modified().ok()?)
            .unwrap_or_default();
        if age > max_age {
            return None;
        }
    }
    std::fs::read_to_string(path).ok()
}

pub fn write(name: &str, text: &str) {
    if let Err(e) = super::atomic_file::write(&paths::cache_file(name), text.as_bytes()) {
        log::warn!("cache write {name} failed: {e}");
    }
}

/// Fresh cache, else download (and cache), else any stale cache.
pub fn fetch_cached(
    name: &str,
    max_age: Duration,
    download: impl FnOnce() -> crate::Result<String>,
) -> crate::Result<String> {
    if let Some(text) = read(name, Some(max_age)) {
        return Ok(text);
    }
    match download() {
        Ok(text) => {
            write(name, &text);
            Ok(text)
        }
        Err(e) => match read(name, None) {
            Some(stale) => {
                log::warn!("using stale cache for {name}: {e}");
                Ok(stale)
            }
            None => Err(e),
        },
    }
}
