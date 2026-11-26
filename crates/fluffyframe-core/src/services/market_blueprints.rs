//! Items whose blueprint the in-game Market sells for credits. DE's public
//! export has no market data, so this reads the community WFCD item data
//! (`bpCost` is set exactly for those items).

use std::collections::HashSet;

use serde_json::Value;

use super::{http, json_cache};
use crate::config::constants::{WFCD_ITEMS_BASE, WFCD_ITEMS_TTL};

/// WFCD category files holding masterable items.
const CATEGORIES: &[&str] =
    &["Warframes", "Primary", "Secondary", "Melee", "Archwing", "Arch-Gun", "Arch-Melee", "Sentinels", "SentinelWeapons", "Pets"];

fn parse(root: &Value, out: &mut HashSet<String>) {
    for item in root.as_array().into_iter().flatten() {
        if item.get("bpCost").is_some_and(|c| c.as_u64().is_some_and(|c| c > 0))
            && let Some(unique) = item.get("uniqueName").and_then(Value::as_str)
        {
            out.insert(unique.to_string());
        }
    }
}

/// uniqueNames of items sold as a Market blueprint. A category that fails to
/// load is skipped (logged), so the rest still apply.
pub fn load() -> HashSet<String> {
    let mut out = HashSet::new();
    for cat in CATEGORIES {
        let url = format!("{WFCD_ITEMS_BASE}{cat}.json");
        let text = json_cache::fetch_cached(&format!("wfcd/{cat}.json"), WFCD_ITEMS_TTL, || http::get_text(&url));
        match text.and_then(|t| Ok(serde_json::from_str::<Value>(&t)?)) {
            Ok(root) => parse(&root, &mut out),
            Err(e) => log::warn!("market blueprints ({cat}): {e:#}"),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn keeps_only_items_with_a_blueprint_cost() {
        let root = serde_json::json!([
            { "uniqueName": "/A", "bpCost": 35000 },
            { "uniqueName": "/B", "marketCost": 150 },
            { "uniqueName": "/C", "bpCost": null },
        ]);
        let mut out = std::collections::HashSet::new();
        super::parse(&root, &mut out);
        assert_eq!(out, ["/A".to_string()].into());
    }
}
