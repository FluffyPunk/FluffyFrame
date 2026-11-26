//! Mastery XP granted per star chart node. DE's live export carries no such
//! value, so this reads the community warframe-public-export-plus regions
//! (`masteryExp`; junctions export 0 but grant a flat 1000 in game).

use std::collections::HashMap;

use serde_json::Value;

use super::{http, json_cache};
use crate::config::constants::{PEP_REGIONS_TTL, PEP_REGIONS_URL};

const JUNCTION_MASTERY_XP: u64 = 1000;

/// What a star chart node grants the first time it's cleared (and again on Steel Path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeMastery {
    pub xp: u64,
    pub junction: bool,
}

fn parse(root: &Value) -> HashMap<String, NodeMastery> {
    root.as_object()
        .into_iter()
        .flatten()
        .map(|(tag, node)| {
            let junction = node.get("missionType").and_then(Value::as_str) == Some("MT_JUNCTION");
            let xp = if junction { JUNCTION_MASTERY_XP } else { node.get("masteryExp").and_then(Value::as_u64).unwrap_or(0) };
            (tag.clone(), NodeMastery { xp, junction })
        })
        .collect()
}

/// Node tag → mastery; empty when the data can't be loaded (logged).
pub fn load() -> HashMap<String, NodeMastery> {
    let text = json_cache::fetch_cached("pep/ExportRegions.json", PEP_REGIONS_TTL, || http::get_text(PEP_REGIONS_URL));
    match text.and_then(|t| Ok(serde_json::from_str::<Value>(&t)?)) {
        Ok(root) => parse(&root),
        Err(e) => {
            log::warn!("node mastery: {e:#}");
            HashMap::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NodeMastery;

    #[test]
    fn junctions_get_a_flat_grant() {
        let root = serde_json::json!({
            "SolNode1": { "missionType": "MT_SURVIVAL", "masteryExp": 24 },
            "AToBJunction": { "missionType": "MT_JUNCTION", "masteryExp": 0 },
        });
        let xp = super::parse(&root);
        assert_eq!(xp["SolNode1"], NodeMastery { xp: 24, junction: false });
        assert_eq!(xp["AToBJunction"], NodeMastery { xp: 1000, junction: true });
    }
}
