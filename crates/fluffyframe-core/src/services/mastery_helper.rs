//! Mastery progress (original: `services/masteryHelper.ts`, `config/shared/masteryXp.ts`).

use std::collections::HashSet;

use chrono::Utc;

use super::foundry;
use super::inventory::{Inventory, MAX_INTRINSIC_RANK, level_from_xp};
use super::item_database::{ItemDb, ItemKind};

/// How an unmastered item can be obtained, best first (the roadmap order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Source {
    /// Built and in the inventory.
    #[default]
    Owned,
    /// Finished in the Foundry, waiting to be claimed.
    ReadyToClaim,
    /// Blueprint owned and every ingredient in hand.
    CraftIt,
    /// Blueprint owned, ingredients (e.g. component blueprints) still to get.
    CraftParts,
    /// Blueprint sold by the Market.
    Market,
    /// Blueprint comes from a clan dojo lab (research needed).
    ClanLab,
    /// No known easy route.
    Other,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Owned => "Owned: level it",
            Source::ReadyToClaim => "Ready to claim",
            Source::CraftIt => "Blueprint: craft it",
            Source::CraftParts => "Blueprint: craft the parts",
            Source::Market => "Blueprint from Market",
            Source::ClanLab => "Blueprint from clan lab",
            Source::Other => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MasteryRow {
    pub unique_name: String,
    pub name: String,
    pub kind: ItemKind,
    pub level: u32,
    pub max_level: u32,
    pub mastered: bool,
    /// Currently in the inventory (built).
    pub owned: bool,
    pub source: Source,
    /// Ingredients in hand / total; `None` once built.
    pub parts: Option<(usize, usize)>,
    pub mastery_req: u32,
}

#[derive(Debug, Clone, Default)]
pub struct MasterySummary {
    pub mastered: usize,
    pub total: usize,
    /// Mastery points from item ranks.
    pub points: u64,
}

/// Mastery points per rank: 200 for frame-like gear, 100 for weapons.
pub fn points_per_rank(kind: ItemKind) -> u64 {
    if kind.is_frame_like() { 200 } else { 100 }
}

/// Total mastery XP needed to reach `rank` (2500·r² up to MR30, then 147,500 per legendary rank).
pub fn mr_xp(rank: u32) -> u64 {
    let r = rank as u64;
    if rank <= 30 { 2500 * r * r } else { 2_250_000 + 147_500 * (r - 30) }
}

/// Where a mastery rank lands after gaining XP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Potential {
    pub rank: u32,
    /// XP past the start of `rank`.
    pub into: u64,
    /// XP between `rank` and the next one.
    pub span: u64,
}

/// Where `total` mastery XP plus `gain` lands.
pub fn potential(total: u64, gain: u64) -> Potential {
    let total = total + gain;
    let mut r = 0;
    while mr_xp(r + 1) <= total {
        r += 1;
    }
    Potential { rank: r, into: total - mr_xp(r), span: mr_xp(r + 1) - mr_xp(r) }
}

/// Mastery XP per intrinsic rank.
const INTRINSIC_MASTERY_XP: u64 = 1500;

/// Total mastery XP: item ranks (`points`) + star chart nodes (once played, again on
/// Steel Path) + intrinsics. Undercounts on partial data, so it never reads below the
/// start of the game's own rank.
pub fn total_xp(inv: &Inventory, db: &ItemDb, points: u64) -> u64 {
    let nodes: u64 = inv
        .nodes
        .iter()
        .filter(|n| n.completes > 0 || n.steel_path)
        .map(|n| db.node_mastery.get(&n.tag).map_or(0, |m| m.xp) * if n.steel_path { 2 } else { 1 })
        .sum();
    let intrinsics: u64 = inv.intrinsics.values().sum();
    (points + nodes + intrinsics * INTRINSIC_MASTERY_XP).max(mr_xp(inv.mastery_rank))
}

/// Done out of total.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

/// Star chart and intrinsic completion (original: `computeStarChartProgress`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChartProgress {
    pub nodes: Progress,
    pub junctions: Progress,
    pub steel_path: Progress,
    pub steel_junctions: Progress,
    /// Intrinsic ranks out of 10 per skill.
    pub railjack: Progress,
    pub drifter: Progress,
}

pub fn chart_progress(inv: &Inventory, db: &ItemDb) -> ChartProgress {
    let mut p = ChartProgress::default();
    // Only nodes that grant mastery count, as in game.
    for m in db.node_mastery.values().filter(|m| m.xp > 0) {
        if m.junction { p.junctions.total += 1 } else { p.nodes.total += 1 }
    }
    p.steel_path.total = p.nodes.total;
    p.steel_junctions.total = p.junctions.total;
    for n in inv.nodes.iter().filter(|n| n.completes > 0 || n.steel_path) {
        let Some(m) = db.node_mastery.get(&n.tag).filter(|m| m.xp > 0) else { continue };
        let (normal, steel) = if m.junction { (&mut p.junctions, &mut p.steel_junctions) } else { (&mut p.nodes, &mut p.steel_path) };
        normal.done += 1;
        if n.steel_path {
            steel.done += 1;
        }
    }
    for (skill, &rank) in &inv.intrinsics {
        let tree = if skill.starts_with("LPS_DRIFT_") { &mut p.drifter } else { &mut p.railjack };
        tree.done += rank;
        tree.total += MAX_INTRINSIC_RANK;
    }
    p
}

pub fn rows(inv: &Inventory, db: &ItemDb) -> (Vec<MasteryRow>, MasterySummary) {
    let counts = inv.count_map();
    let now = Utc::now();
    let ready: HashSet<&str> = inv
        .pending_recipes
        .iter()
        .filter(|p| p.completes_at.is_some_and(|t| t <= now))
        .filter_map(|p| db.recipes.get(&p.item_type))
        .map(|r| r.result_type.as_str())
        .collect();
    let mut summary = MasterySummary::default();
    let mut rows: Vec<MasteryRow> = db
        .items
        .values()
        .filter(|i| i.masterable && !i.name.is_empty())
        .map(|i| {
            let xp = inv.xp_info.get(&i.unique_name).copied().unwrap_or(0);
            let level = level_from_xp(xp, i.kind.is_frame_like(), i.max_level);
            let mastered = level >= i.max_level;
            summary.total += 1;
            summary.points += level as u64 * points_per_rank(i.kind);
            if mastered {
                summary.mastered += 1;
            }
            let owned = counts.get(i.unique_name.as_str()).copied().unwrap_or(0) > 0;
            let recipe = db.recipe_by_result.get(&i.unique_name);
            // Ingredients in hand / needed, for items still to be built.
            let parts = recipe
                .and_then(|r| db.recipes.get(r))
                .map(|r| foundry::ingredient_rows(r, db, &counts))
                .map(|g| (g.iter().filter(|g| g.have >= g.need).count(), g.len()))
                .filter(|_| !owned);
            let has_blueprint = recipe.is_some_and(|r| counts.get(r.as_str()).copied().unwrap_or(0) > 0);
            let source = if owned {
                Source::Owned
            } else if ready.contains(i.unique_name.as_str()) {
                Source::ReadyToClaim
            } else if has_blueprint {
                match parts {
                    Some((have, total)) if have < total => Source::CraftParts,
                    _ => Source::CraftIt,
                }
            } else if recipe.is_some_and(|r| r.contains("/ClanTech/")) {
                Source::ClanLab
            } else if db.market_blueprints.contains(&i.unique_name) {
                Source::Market
            } else {
                Source::Other
            };
            MasteryRow {
                owned,
                source,
                parts,
                unique_name: i.unique_name.clone(),
                name: i.name.clone(),
                kind: i.kind,
                max_level: i.max_level,
                mastery_req: i.mastery_req,
                level,
                mastered,
            }
        })
        .collect();
    // Two masterables that aren't in the masterable list but earn frame-rate
    // mastery (as in the original): Khora's Venari and Railjack harnesses.
    for (unique, &xp) in &inv.xp_info {
        let extra = unique.ends_with("Harness") || (unique.contains("/Khora/Kavat/") && unique.ends_with("KavatPowerSuit"));
        if extra && !db.items.get(unique).is_some_and(|i| i.masterable) {
            summary.points += level_from_xp(xp, true, 30) as u64 * points_per_rank(ItemKind::Warframe);
        }
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    (rows, summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potential_crosses_ranks() {
        assert_eq!(mr_xp(30), 2_250_000);
        assert_eq!(mr_xp(31), 2_397_500);
        let p = potential(mr_xp(23), mr_xp(25) - mr_xp(23) + 100);
        assert_eq!((p.rank, p.into, p.span), (25, 100, mr_xp(26) - mr_xp(25)));
    }

    #[test]
    fn chart_progress_counts_mastery_nodes() {
        use super::super::inventory::NodeProgress;
        use super::super::region_mastery::NodeMastery;
        let mut db = ItemDb::default();
        for (tag, xp, junction) in [("A", 24, false), ("B", 51, false), ("J", 1000, true), ("Hub", 0, false)] {
            db.node_mastery.insert(tag.into(), NodeMastery { xp, junction });
        }
        let mut inv = Inventory::default();
        for (tag, completes, steel_path) in [("A", 3, true), ("J", 1, false), ("Hub", 9, false), ("B", 0, false)] {
            inv.nodes.push(NodeProgress { tag: tag.into(), completes, steel_path });
        }
        inv.intrinsics = [("LPS_PILOTING", 10), ("LPS_COMMAND", 4), ("LPS_DRIFT_RIDING", 8)].map(|(k, r)| (k.to_string(), r)).into();
        let p = chart_progress(&inv, &db);
        let pair = |p: Progress| (p.done, p.total);
        assert_eq!(pair(p.nodes), (1, 2));
        assert_eq!(pair(p.junctions), (1, 1));
        assert_eq!(pair(p.steel_path), (1, 2));
        assert_eq!(pair(p.steel_junctions), (0, 1));
        assert_eq!(pair(p.railjack), (14, 20));
        assert_eq!(pair(p.drifter), (8, 10));
        // 24·2 (A, also on Steel Path) + 1000 (J) + 22 intrinsic ranks · 1500
        assert_eq!(total_xp(&inv, &db, 0), 48 + 1000 + 22 * 1500);
    }
}
