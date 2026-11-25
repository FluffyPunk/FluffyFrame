//! Live DE public export (original: `services/publicExportSource.ts`).
//!
//! DE serves a tiny LZMA index of hash-suffixed manifest names, then plain-JSON
//! manifests under `/Manifest/`. Manifests are cached under their hashed name, so a
//! new game update invalidates exactly the files that changed.

use std::collections::HashMap;

use anyhow::Context;
use serde_json::Value;

use super::{http, json_cache};
use crate::config::constants::{PUBLIC_EXPORT_INDEX_URL, PUBLIC_EXPORT_MANIFEST_BASE};

/// Manifests the app reads. Cosmetics (ExportCustoms/ExportFlavour) are left out.
pub const WANTED: &[&str] = &[
    "ExportWarframes",
    "ExportWeapons",
    "ExportSentinels",
    "ExportUpgrades",
    "ExportResources",
    "ExportRelicArcane",
    "ExportRecipes",
    "ExportGear",
    "ExportKeys",
    "ExportDrones",
    "ExportFusionBundles",
    "ExportRegions",
    "ExportManifest",
];

const INDEX_CACHE: &str = "export/index.txt";

/// Parsed manifests keyed by export name (`ExportWeapons`, …).
pub struct ExportSet {
    pub files: HashMap<String, Value>,
}

impl ExportSet {
    /// The array under `key` inside manifest `file` (e.g. `ExportRelicArcane`).
    pub fn array<'a>(&'a self, file: &str, key: &str) -> &'a [Value] {
        self.files
            .get(file)
            .and_then(|v| v.get(key))
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// `ExportWeapons_en.json!00_hash` → (`ExportWeapons`, the full line).
pub fn parse_index(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(|line| {
            let file = line.split('!').next()?;
            let stem = file.strip_suffix(".json")?;
            let name = stem.strip_suffix("_en").unwrap_or(stem);
            Some((name.to_string(), line.to_string()))
        })
        .collect()
}

fn load_index() -> crate::Result<String> {
    match http::get_bytes(PUBLIC_EXPORT_INDEX_URL).and_then(|bytes| {
        let mut out = Vec::new();
        lzma_rs::lzma_decompress(&mut bytes.as_slice(), &mut out)
            .map_err(|e| anyhow::anyhow!("lzma: {e:?}"))?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    }) {
        Ok(text) => {
            json_cache::write(INDEX_CACHE, &text);
            Ok(text)
        }
        Err(e) => {
            log::warn!("public export index unavailable ({e}), using cached index");
            json_cache::read(INDEX_CACHE, None).context("no cached public export index")
        }
    }
}

fn cache_name(line: &str) -> String {
    let safe: String = line
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' { c } else { '_' })
        .collect();
    format!("export/{safe}")
}

/// Downloads (or reads from cache) every wanted manifest.
pub fn load(progress: &dyn Fn(&str)) -> crate::Result<ExportSet> {
    let index = load_index()?;
    let mut files = HashMap::new();
    for (name, line) in parse_index(&index) {
        if !WANTED.contains(&name.as_str()) {
            continue;
        }
        progress(&name);
        let cache = cache_name(&line);
        let text = match json_cache::read(&cache, None) {
            Some(text) => text,
            None => {
                let url = format!("{PUBLIC_EXPORT_MANIFEST_BASE}{line}");
                let text = http::strip_control_chars(&http::get_text(&url)?);
                json_cache::write(&cache, &text);
                text
            }
        };
        let value: Value =
            serde_json::from_str(&text).with_context(|| format!("parsing {name}"))?;
        files.insert(name, value);
    }
    anyhow::ensure!(!files.is_empty(), "public export returned no manifests");
    Ok(ExportSet { files })
}

#[cfg(test)]
mod tests {
    #[test]
    fn index_lines() {
        let parsed = super::parse_index(
            "ExportWeapons_en.json!00_Lp7t\r\nExportManifest.json!00_y10Y\r\n",
        );
        assert_eq!(parsed[0].0, "ExportWeapons");
        assert_eq!(parsed[1].0, "ExportManifest");
        assert_eq!(parsed[1].1, "ExportManifest.json!00_y10Y");
    }
}
