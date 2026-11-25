//! Crash-safe writes (original: `services/atomicFile.ts`): write a sibling temp
//! file, then rename over the target.

use std::io::Write;
use std::path::Path;

pub fn write(path: &Path, bytes: &[u8]) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    {
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Writes JSON with two-space indentation.
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> crate::Result<()> {
    write(path, serde_json::to_string_pretty(value)?.as_bytes())
}

/// Reads JSON, `None` on a missing or malformed file.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.json");
        super::write_json(&path, &vec![1, 2, 3]).unwrap();
        let back: Vec<i32> = super::read_json(&path).unwrap();
        assert_eq!(back, vec![1, 2, 3]);
    }
}
