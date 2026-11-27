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
    bounties: Vec<BountyTable>,
}

/// One bounty reward pool: a level bracket of one faction's bounties.
#[derive(Debug, Clone)]
pub struct BountyTable {
    /// Drop data list it came from (see [`bounty_source`]).
    pub source: &'static str,
    /// What the bracket is, after its levels: "Cetus Bounty", "Isolation Vault".
    pub label: String,
    pub min_level: u32,
    pub max_level: u32,
    /// Rotation letter → rewards, best chance first; empty rotations left out.
    pub rotations: Vec<(char, Vec<BountyReward>)>,
}

#[derive(Debug, Clone)]
pub struct BountyReward {
    pub item: String,
    pub rarity: String,
    /// "Stage 1", "Final Stage", …
    pub stage: String,
    pub chance: f64,
}

/// Bounty reward lists in the drop data, one per faction.
const BOUNTY_SOURCES: [&str; 6] =
    ["cetusBountyRewards", "solarisBountyRewards", "deimosRewards", "zarimanRewards", "entratiLabRewards", "hexRewards"];

/// The drop data list for a syndicate's bounties.
pub fn bounty_source(tag: &str) -> Option<&'static str> {
    let i = ["CetusSyndicate", "SolarisSyndicate", "EntratiSyndicate", "ZarimanSyndicate", "EntratiLabSyndicate", "HexSyndicate"]
        .iter()
        .position(|t| *t == tag)?;
    Some(BOUNTY_SOURCES[i])
}

/// "Level  50 - 55 Zariman Bounty" → (50, 55, "Zariman Bounty").
fn bounty_level(name: &str) -> Option<(u32, u32, String)> {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.as_slice() {
        ["Level", lo, "-", hi, rest @ ..] => Some((lo.parse().ok()?, hi.parse().ok()?, rest.join(" "))),
        _ => None,
    }
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

    /// A faction's bounty brackets, in drop data order.
    pub fn bounty_tables(&self, source: &'static str) -> impl Iterator<Item = &BountyTable> {
        self.bounties.iter().filter(move |t| t.source == source)
    }

    /// The table a live bounty pays from: same faction and levels. Vault jobs pay
    /// from the Isolation Vault tables, which share levels with regular bounties.
    pub fn bounty_for(&self, source: &'static str, min_level: u32, max_level: u32, vault: bool) -> Option<&BountyTable> {
        self.bounty_tables(source)
            .filter(|t| (t.min_level, t.max_level) == (min_level, max_level))
            .find(|t| (t.label == "Isolation Vault") == vault)
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
        for key in BOUNTY_SOURCES {
            for bounty in root.get(key).and_then(Value::as_array).into_iter().flatten() {
                let level = s(bounty, "bountyLevel");
                let mut rotations = Vec::new();
                for (rot, list) in bounty.get("rewards").and_then(Value::as_object).into_iter().flatten() {
                    let mut rewards = Vec::new();
                    for r in list.as_array().into_iter().flatten() {
                        idx.add(&s(r, "itemName"), level.clone(), format!("Rotation {rot}, {}", s(r, "stage")), chance(r));
                        rewards.push(BountyReward { item: s(r, "itemName"), rarity: s(r, "rarity"), stage: s(r, "stage"), chance: chance(r) });
                    }
                    if let (Some(letter), false) = (rot.chars().next(), rewards.is_empty()) {
                        rotations.push((letter, rewards));
                    }
                }
                if let Some((min_level, max_level, label)) = bounty_level(&level) {
                    idx.bounties.push(BountyTable { source: key, label, min_level, max_level, rotations });
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

    #[test]
    fn bounties_match_live_jobs() {
        let reward = |item: &str| serde_json::json!({ "itemName": item, "rarity": "Rare", "chance": 10.0, "stage": "Final Stage" });
        let root = serde_json::json!({ "deimosRewards": [
            { "bountyLevel": "Level 30 - 40 Cambion Drift Bounty", "rewards": { "A": [reward("Bounty A")], "B": [reward("Bounty B")], "C": [] } },
            { "bountyLevel": "Level 30 - 40 Isolation Vault", "rewards": { "A": [reward("Vault A")] } },
        ], "zarimanRewards": [
            { "bountyLevel": "Level  50 - 55 Zariman Bounty", "rewards": { "A": [], "C": [reward("Voidplume")] } },
        ] });
        let idx = super::DropIndex::parse(&root);
        let deimos = super::bounty_source("EntratiSyndicate").unwrap();
        let bounty = idx.bounty_for(deimos, 30, 40, false).unwrap();
        assert_eq!(bounty.label, "Cambion Drift Bounty");
        assert_eq!(bounty.rotations.iter().map(|r| r.0).collect::<String>(), "AB");
        assert_eq!(idx.bounty_for(deimos, 30, 40, true).unwrap().rotations[0].1[0].item, "Vault A");
        let zariman: Vec<_> = idx.bounty_tables(super::bounty_source("ZarimanSyndicate").unwrap()).collect();
        assert_eq!((zariman[0].min_level, zariman[0].max_level, zariman[0].rotations[0].0), (50, 55, 'C'));
    }
}
