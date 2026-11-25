//! Inventory import/export (original: `services/alecaFrameInventory.ts`,
//! `config/shared/inventorySource.ts`, the Settings export action).

use std::path::Path;

use aes::Aes128;
use cbc::cipher::block_padding::Pkcs7;
use cbc::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::atomic_file;
use super::inventory::Inventory;
use crate::config::paths;
use crate::config::settings::InventorySource;

// The AlecaFrame cache key/IV, as used by warframe-api-helper and wf-api-rust.
const KEY: [u8; 16] = [76, 69, 79, 45, 65, 76, 69, 67, 9, 69, 79, 45, 65, 76, 69, 67];
const IV: [u8; 16] = [49, 50, 70, 71, 66, 51, 54, 45, 76, 69, 51, 45, 113, 61, 57, 0];

type Dec = cbc::Decryptor<Aes128>;
type Enc = cbc::Encryptor<Aes128>;

pub fn decrypt_alecaframe(bytes: &[u8]) -> crate::Result<String> {
    let plain = Dec::new(&KEY.into(), &IV.into())
        .decrypt_padded_vec::<Pkcs7>(bytes)
        .map_err(|_| anyhow::anyhow!("not an AlecaFrame lastData.dat cache (decryption failed)"))?;
    Ok(String::from_utf8_lossy(&plain).into_owned())
}

pub fn encrypt_alecaframe(text: &str) -> Vec<u8> {
    Enc::new(&KEY.into(), &IV.into()).encrypt_padded_vec::<Pkcs7>(text.as_bytes())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub source: InventorySource,
    pub fetched_at: chrono::DateTime<Utc>,
}

/// The last loaded inventory text plus where it came from.
pub fn save_snapshot(text: &str, source: InventorySource) -> crate::Result<()> {
    atomic_file::write(&paths::inventory_snapshot_file(), text.as_bytes())?;
    atomic_file::write_json(
        &paths::user_file("inventory-meta.json"),
        &SnapshotMeta { source, fetched_at: Utc::now() },
    )
}

pub fn load_snapshot() -> Option<(Inventory, SnapshotMeta)> {
    let text = std::fs::read_to_string(paths::inventory_snapshot_file()).ok()?;
    let meta: SnapshotMeta = atomic_file::read_json(&paths::user_file("inventory-meta.json"))?;
    let mut inv = Inventory::parse_text(&text).ok()?;
    inv.source = Some(meta.source);
    inv.fetched_at = Some(meta.fetched_at);
    Some((inv, meta))
}

/// Imports a user-picked file. AlecaFrame caches are detected by failing to parse as JSON.
pub fn import_file(path: &Path) -> crate::Result<(Inventory, InventorySource)> {
    let bytes = std::fs::read(path)?;
    let (text, source) = match std::str::from_utf8(&bytes)
        .ok()
        .filter(|t| t.trim_start().starts_with('{'))
    {
        Some(t) => (t.to_string(), InventorySource::ImportedJson),
        None => (decrypt_alecaframe(&bytes)?, InventorySource::AlecaFrame),
    };
    let mut inv = Inventory::parse_text(&text)?;
    save_snapshot(&text, source)?;
    inv.source = Some(source);
    inv.fetched_at = Some(Utc::now());
    Ok((inv, source))
}

/// Writes the loaded snapshot as indented JSON.
pub fn export_to(path: &Path) -> crate::Result<()> {
    let text = std::fs::read_to_string(paths::inventory_snapshot_file())?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    atomic_file::write_json(path, &value)
}

#[cfg(test)]
mod tests {
    #[test]
    fn alecaframe_round_trip() {
        let text = r#"{"MiscItems":[]}"#;
        let enc = super::encrypt_alecaframe(text);
        assert_eq!(super::decrypt_alecaframe(&enc).unwrap(), text);
        assert!(super::decrypt_alecaframe(b"garbage-not-a-multiple-of-16").is_err());
    }
}
