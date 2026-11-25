//! Drop locations (original: `services/dropData.ts`, `src/lib/resolveDrops.ts`).
//!
//! Indexes drops.warframestat.us by item name for the Wiki view.

use std::collections::HashMap;

use serde_json::Value;

use super::{http, json_cache};
use crate::config::constants::{DROPS_TTL, DROPS_URL};

#[derive(Debug, Clone)]
pub struct DropSource {
    pub location: String,
    pub detail: String,
    pub chance: f64,
}

#[derive(Default)]
pub struct DropIndex {
    by_item: HashMap<String, Vec<DropSource>>,
}

impl DropIndex {
    pub fn lookup(&self, item_name: &str) -> &[DropSource] {
        self.by_item.get(&item_name.trim().to_lowercase()).map(Vec::as_slice).unwrap_or(&[])
    }

    /// (item, source) pairs whose location matches `query`, best chance first.
    pub fn by_location(&self, query: &str, limit: usize) -> Vec<(&str, &DropSource)> {
        let q = query.trim().to_lowercase();
        if q.len() < 2 {
            return Vec::new();
        }
        let mut hits: Vec<(&str, &DropSource)> = self
            .by_item
            .iter()
            .flat_map(|(item, list)| list.iter().map(move |s| (item.as_str(), s)))
            .filter(|(_, s)| s.location.to_lowercase().contains(&q))
            .collect();
        hits.sort_by(|a, b| a.1.location.cmp(&b.1.location).then(b.1.chance.total_cmp(&a.1.chance)));
        hits.truncate(limit);
        hits
    }

    fn add(&mut self, item: &str, location: String, detail: String, chance: f64) {
        self.by_item
            .entry(item.trim().to_lowercase())
            .or_default()
            .push(DropSource { location, detail, chance });
    }

    pub fn parse(root: &Value) -> Self {
        let mut idx = DropIndex::default();
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let chance = |v: &Value| v.get("chance").and_then(Value::as_f64).unwrap_or(0.0);

        // Star chart: planet → node → { gameMode, rewards: [..] | {A,B,C} }
        if let Some(planets) = root.get("missionRewards").and_then(Value::as_object) {
            for (planet, nodes) in planets {
                for (node, data) in nodes.as_object().into_iter().flatten() {
                    let mode = s(data, "gameMode");
                    let place = format!("{planet}/{node} ({mode})");
                    match data.get("rewards") {
                        Some(Value::Array(list)) => {
                            for r in list {
                                idx.add(&s(r, "itemName"), place.clone(), String::new(), chance(r));
                            }
                        }
                        Some(Value::Object(rots)) => {
                            for (rot, list) in rots {
                                for r in list.as_array().into_iter().flatten() {
                                    idx.add(&s(r, "itemName"), place.clone(), format!("Rotation {rot}"), chance(r));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        for t in root.get("transientRewards").and_then(Value::as_array).into_iter().flatten() {
            let obj = s(t, "objectiveName");
            for r in t.get("rewards").and_then(Value::as_array).into_iter().flatten() {
                let rot = s(r, "rotation");
                idx.add(&s(r, "itemName"), obj.clone(), if rot.is_empty() { rot } else { format!("Rotation {rot}") }, chance(r));
            }
        }
        for m in root.get("modLocations").and_then(Value::as_array).into_iter().flatten() {
            let name = s(m, "modName");
            for e in m.get("enemies").and_then(Value::as_array).into_iter().flatten() {
                let drop = e.get("enemyModDropChance").and_then(Value::as_f64).unwrap_or(100.0) / 100.0;
                idx.add(&name, s(e, "enemyName"), "Enemy drop".into(), chance(e) * drop);
            }
        }
        for b in root.get("blueprintLocations").and_then(Value::as_array).into_iter().flatten() {
            let name = s(b, "itemName");
            for e in b.get("enemies").and_then(Value::as_array).into_iter().flatten() {
                let drop = e.get("enemyItemDropChance").and_then(Value::as_f64).unwrap_or(100.0) / 100.0;
                idx.add(&name, s(e, "enemyName"), "Enemy drop".into(), chance(e) * drop);
            }
        }
        for key in [
            "cetusBountyRewards",
            "solarisBountyRewards",
            "deimosRewards",
            "zarimanRewards",
            "entratiLabRewards",
            "hexRewards",
        ] {
            for bounty in root.get(key).and_then(Value::as_array).into_iter().flatten() {
                let level = s(bounty, "bountyLevel");
                for (rot, list) in bounty.get("rewards").and_then(Value::as_object).into_iter().flatten() {
                    for r in list.as_array().into_iter().flatten() {
                        idx.add(&s(r, "itemName"), level.clone(), format!("Rotation {rot}, {}", s(r, "stage")), chance(r));
                    }
                }
            }
        }
        for r in root.get("sortieRewards").and_then(Value::as_array).into_iter().flatten() {
            idx.add(&s(r, "itemName"), "Sortie".into(), String::new(), chance(r));
        }
        for list in idx.by_item.values_mut() {
            list.sort_by(|a, b| b.chance.total_cmp(&a.chance));
        }
        idx
    }

    pub fn load() -> crate::Result<Self> {
        let text = json_cache::fetch_cached("drops-all.json", DROPS_TTL, || http::get_text(DROPS_URL))?;
        Ok(Self::parse(&serde_json::from_str(&text)?))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn indexes_star_chart() {
        let root = serde_json::json!({ "missionRewards": { "Mercury": { "Apollodorus": {
            "gameMode": "Survival", "rewards": { "A": [{ "itemName": "Lith A1 Relic", "chance": 12.5 }] } } } } });
        let idx = super::DropIndex::parse(&root);
        let hits = idx.lookup("lith a1 relic");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].detail, "Rotation A");
    }
}
