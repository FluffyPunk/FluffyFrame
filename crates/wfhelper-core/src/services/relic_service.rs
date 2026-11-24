//! Owned relics and their expected value (original: `services/relicService.ts`,
//! `src/lib/relic/*`). The in-game relic planner overlay is not ported; its
//! ranking (expected platinum per run) lives here for the Relics view.

use std::collections::HashMap;

use super::inventory::Inventory;
use super::item_database::{ItemDb, Rarity, Refinement, RelicEra, RelicInfo};

#[derive(Debug, Clone)]
pub struct OwnedRelic {
    pub family: String,
    pub name: String,
    pub era: RelicEra,
    /// Count per refinement, indexed like [`Refinement::ALL`].
    pub counts: [u64; 4],
    pub vaulted: bool,
}

impl OwnedRelic {
    pub fn total(&self) -> u64 {
        self.counts.iter().sum()
    }
}

/// The intact entry of each relic family.
pub fn intact_by_family(db: &ItemDb) -> HashMap<&str, &RelicInfo> {
    db.relics
        .values()
        .filter(|r| r.refinement == Refinement::Intact)
        .map(|r| (r.family.as_str(), r))
        .collect()
}

pub fn owned_relics(inv: &Inventory, db: &ItemDb) -> Vec<OwnedRelic> {
    let mut by_family: HashMap<String, OwnedRelic> = HashMap::new();
    for item in &inv.items {
        let Some(relic) = db.relics.get(&item.unique_name) else { continue };
        let idx = Refinement::ALL.iter().position(|r| *r == relic.refinement).unwrap_or(0);
        let entry = by_family.entry(relic.family.clone()).or_insert_with(|| OwnedRelic {
            family: relic.family.clone(),
            name: relic.name.clone(),
            era: relic.era,
            counts: [0; 4],
            vaulted: db.get(&item.unique_name).is_some_and(|i| i.vaulted),
        });
        entry.counts[idx] += item.count;
    }
    let mut out: Vec<OwnedRelic> = by_family.into_values().collect();
    out.sort_by_key(|a| (a.era, natural_key(&a.name)));
    out
}

/// "Axi A10" sorts after "Axi A9".
fn natural_key(name: &str) -> (String, u32) {
    let code = name.split_whitespace().nth(1).unwrap_or("");
    let letters: String = code.chars().take_while(|c| c.is_alphabetic()).collect();
    let digits: u32 = code.chars().skip_while(|c| c.is_alphabetic()).collect::<String>().parse().unwrap_or(0);
    (letters, digits)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RelicValue {
    /// Expected platinum of one opening, solo.
    pub platinum: f64,
    pub ducats: f64,
    /// Rewards with no price yet.
    pub unpriced: usize,
}

/// Expected value of a solo opening at `refinement`.
pub fn expected_value(
    relic: &RelicInfo,
    refinement: Refinement,
    db: &ItemDb,
    price_of: &dyn Fn(&str) -> Option<f64>,
) -> RelicValue {
    let mut v = RelicValue::default();
    for reward in &relic.rewards {
        let chance = reward.rarity.chance(refinement);
        match price_of(&reward.unique_name) {
            Some(p) => v.platinum += chance * p * reward.count as f64,
            None if !reward.unique_name.contains("Forma") => v.unpriced += 1,
            None => {}
        }
        if let Some(d) = db.get(&reward.unique_name).and_then(|i| i.ducats) {
            v.ducats += chance * d as f64 * reward.count as f64;
        }
    }
    v
}

/// Per-reward share of an opening's expected value.
#[derive(Debug, Clone, Copy, Default)]
pub struct RewardEv {
    /// Probability you end up taking this reward.
    pub probability: f64,
    pub platinum: f64,
    pub ducats: f64,
}

/// Squad expected value (original: the relic modal's Solo/2P/3P/4P switch).
///
/// With `squad` players cracking the same relic you pick the best of `squad`
/// rolls. Ordering rewards by value, the chance the best roll is reward `i` is
/// `F(i)^n - F(i-1)^n`, where `F` is the cumulative drop chance. Platinum and
/// ducats are ranked separately (you pick for one or the other).
pub fn squad_value(
    relic: &RelicInfo,
    refinement: Refinement,
    squad: u32,
    db: &ItemDb,
    price_of: &dyn Fn(&str) -> Option<f64>,
) -> (RelicValue, Vec<RewardEv>) {
    let n = squad.max(1) as i32;
    // The published chances are rounded (Intact sums to 0.9999); normalise them.
    let total_chance: f64 = relic.rewards.iter().map(|w| w.rarity.chance(refinement)).sum::<f64>().max(1e-9);
    let rows: Vec<(f64, f64, f64)> = relic
        .rewards
        .iter()
        .map(|w| {
            let chance = w.rarity.chance(refinement) / total_chance;
            let plat = price_of(&w.unique_name).unwrap_or(0.0) * w.count as f64;
            let ducats = db.get(&w.unique_name).and_then(|i| i.ducats).unwrap_or(0) as f64 * w.count as f64;
            (chance, plat, ducats)
        })
        .collect();
    let best_of = |key: &dyn Fn(&(f64, f64, f64)) -> f64| -> Vec<f64> {
        let mut order: Vec<usize> = (0..rows.len()).collect();
        order.sort_by(|&a, &b| key(&rows[a]).total_cmp(&key(&rows[b])));
        let mut p = vec![0.0; rows.len()];
        let mut cum = 0.0f64;
        for i in order {
            let before = cum.powi(n);
            cum += rows[i].0;
            p[i] = cum.min(1.0).powi(n) - before;
        }
        p
    };
    let p_plat = best_of(&|r| r.1);
    let p_duc = best_of(&|r| r.2);
    let mut total = RelicValue::default();
    let per: Vec<RewardEv> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            total.platinum += p_plat[i] * r.1;
            total.ducats += p_duc[i] * r.2;
            if r.1 == 0.0 && !relic.rewards[i].unique_name.contains("Forma") {
                total.unpriced += 1;
            }
            RewardEv { probability: p_plat[i], platinum: p_plat[i] * r.1, ducats: p_duc[i] * r.2 }
        })
        .collect();
    (total, per)
}

/// Rewards ordered rare → common, the order the game lists them.
pub fn sorted_rewards(relic: &RelicInfo) -> Vec<&super::item_database::RelicReward> {
    let mut r: Vec<_> = relic.rewards.iter().collect();
    r.sort_by_key(|w| std::cmp::Reverse(w.rarity));
    r
}

pub fn rarity_chance_label(rarity: Rarity, refinement: Refinement) -> String {
    format!("{:.1}%", rarity.chance(refinement) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::item_database::RelicReward;

    #[test]
    fn natural_sort() {
        assert!(natural_key("Axi A9") < natural_key("Axi A10"));
    }

    #[test]
    fn chances_sum_to_one() {
        for r in Refinement::ALL {
            let (c, u, rare) = r.chances();
            assert!((3.0 * c + 2.0 * u + rare - 1.0).abs() < 0.001, "{r:?}");
        }
    }

    #[test]
    fn squad_ev_grows_with_squad() {
        let relic = RelicInfo {
            unique_name: "x".into(),
            name: "Axi A1".into(),
            era: RelicEra::Axi,
            refinement: Refinement::Intact,
            family: "x".into(),
            rewards: vec![
                RelicReward { unique_name: "c".into(), rarity: Rarity::Common, count: 1 },
                RelicReward { unique_name: "c2".into(), rarity: Rarity::Common, count: 1 },
                RelicReward { unique_name: "c3".into(), rarity: Rarity::Common, count: 1 },
                RelicReward { unique_name: "u".into(), rarity: Rarity::Uncommon, count: 1 },
                RelicReward { unique_name: "u2".into(), rarity: Rarity::Uncommon, count: 1 },
                RelicReward { unique_name: "rare".into(), rarity: Rarity::Rare, count: 1 },
            ],
        };
        let price = |u: &str| Some(if u == "rare" { 100.0 } else { 1.0 });
        let db = ItemDb::default();
        let (solo, per) = squad_value(&relic, Refinement::Intact, 1, &db, &price);
        let (four, _) = squad_value(&relic, Refinement::Intact, 4, &db, &price);
        let psum: f64 = per.iter().map(|r| r.probability).sum();
        assert!((psum - 1.0).abs() < 1e-3);
        assert!((solo.platinum - expected_value(&relic, Refinement::Intact, &db, &price).platinum).abs() < 1e-3);
        // Best of four: 1 - 0.98^4 chance of the rare.
        let p_rare = 1.0 - 0.98f64.powi(4);
        assert!((four.platinum - (p_rare * 100.0 + (1.0 - p_rare) * 1.0)).abs() < 1e-3);
    }

    #[test]
    fn ev() {
        let relic = RelicInfo {
            unique_name: "x".into(),
            name: "Axi A1".into(),
            era: RelicEra::Axi,
            refinement: Refinement::Intact,
            family: "x".into(),
            rewards: vec![RelicReward { unique_name: "rare".into(), rarity: Rarity::Rare, count: 1 }],
        };
        let v = expected_value(&relic, Refinement::Radiant, &ItemDb::default(), &|_| Some(100.0));
        assert!((v.platinum - 10.0).abs() < 1e-9);
    }
}
