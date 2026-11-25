//! Disk cache for remote item art, so each image downloads once instead of on
//! every launch. Art URLs are content-stable (wiki card stems, DE texture
//! paths), so entries never expire; deleting `cache/images/` forces a refresh.

use std::path::PathBuf;

use super::{atomic_file, http};
use crate::config::paths;

/// Cached bytes for `url`, if it was downloaded before.
pub fn read(url: &str) -> Option<Vec<u8>> {
    std::fs::read(path_for(url)).ok()
}

/// Downloads `url` and stores it for later launches. A failed disk write only
/// costs a re-download next time, so it is logged rather than returned.
pub fn fetch(url: &str) -> crate::Result<Vec<u8>> {
    let bytes = http::get_bytes(url)?;
    if let Err(err) = atomic_file::write(&path_for(url), &bytes) {
        log::warn!("image cache write failed for {url}: {err:#}");
    }
    Ok(bytes)
}

fn path_for(url: &str) -> PathBuf {
    paths::cache_file(&format!("images/{:016x}.img", fnv1a(url.as_bytes())))
}

/// FNV-1a: stable across Rust versions, unlike `DefaultHasher`, so cache file
/// names survive toolchain upgrades.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

#[cfg(test)]
mod tests {
    #[test]
    fn hash_is_stable_and_distinct() {
        assert_eq!(super::fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(super::fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(super::fnv1a(b"https://x/a.png"), super::fnv1a(b"https://x/b.png"));
    }
}
