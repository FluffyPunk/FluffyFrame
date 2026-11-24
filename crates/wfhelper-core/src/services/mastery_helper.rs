//! Mastery progress (original: `services/masteryHelper.ts`, `config/shared/masteryXp.ts`).

use super::inventory::{Inventory, level_from_xp};
use super::item_database::{ItemDb, ItemKind};

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
    /// A blueprint or parts for it are in the inventory.
    pub craftable_now: bool,
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

pub fn rows(inv: &Inventory, db: &ItemDb) -> (Vec<MasteryRow>, MasterySummary) {
    let counts = inv.count_map();
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
            let blueprint = db.recipe_by_result.get(&i.unique_name);
            MasteryRow {
                owned: counts.get(i.unique_name.as_str()).copied().unwrap_or(0) > 0,
                craftable_now: blueprint.and_then(|b| counts.get(b.as_str())).copied().unwrap_or(0) > 0,
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
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    (rows, summary)
}
