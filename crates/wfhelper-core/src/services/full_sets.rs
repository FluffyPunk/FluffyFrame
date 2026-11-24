//! Complete sets from owned parts (original: `src/lib/inventory/fullSets.ts`).
//!
//! A set is a main blueprint plus each crafted component it consumes. A part counts
//! as owned in its blueprint or built form.

use std::collections::HashMap;

use super::inventory::Inventory;
use super::item_database::{ItemDb, ItemKind};
use super::wfm_catalog::Catalog;

#[derive(Debug, Clone)]
pub struct SetPart {
    pub name: String,
    pub owned: u64,
    pub needed: u64,
}

#[derive(Debug, Clone)]
pub struct FullSet {
    pub name: String,
    pub result: String,
    pub slug: Option<String>,
    pub parts: Vec<SetPart>,
    pub complete: u64,
}

impl FullSet {
    pub fn missing(&self) -> Vec<&SetPart> {
        self.parts.iter().filter(|p| p.owned < p.needed).collect()
    }
}

/// Sets for every tradable main blueprint (`include_incomplete` keeps partial ones
/// with at least one owned part).
pub fn compute(inv: &Inventory, db: &ItemDb, catalog: &Catalog, include_incomplete: bool) -> Vec<FullSet> {
    let counts: HashMap<&str, u64> = inv.count_map();
    let mut out = Vec::new();
    for recipe in db.recipes.values() {
        let Some(result) = db.get(&recipe.result_type) else { continue };
        if !result.kind.is_equipment() || catalog.by_game_ref(&recipe.unique_name).is_none() {
            continue;
        }
        let mut parts = vec![SetPart {
            name: db.name(&recipe.unique_name),
            owned: counts.get(recipe.unique_name.as_str()).copied().unwrap_or(0),
            needed: 1,
        }];
        for ing in &recipe.ingredients {
            let is_component = db
                .get(&ing.item_type)
                .is_some_and(|i| matches!(i.kind, ItemKind::Component) || i.kind.is_equipment());
            if !is_component {
                continue;
            }
            let blueprint = db.recipe_by_result.get(&ing.item_type);
            let owned = counts.get(ing.item_type.as_str()).copied().unwrap_or(0)
                + blueprint.and_then(|b| counts.get(b.as_str())).copied().unwrap_or(0);
            // Name the part the way the market lists it (blueprint form when tradable).
            let name = blueprint
                .filter(|b| catalog.by_game_ref(b).is_some())
                .map(|b| db.name(b))
                .unwrap_or_else(|| db.name(&ing.item_type));
            parts.push(SetPart { name, owned, needed: ing.count as u64 });
        }
        if parts.len() < 2 {
            continue;
        }
        let complete = parts.iter().map(|p| p.owned / p.needed.max(1)).min().unwrap_or(0);
        let any_owned = parts.iter().any(|p| p.owned > 0);
        if complete == 0 && !(include_incomplete && any_owned) {
            continue;
        }
        let name = format!("{} Set", result.name);
        out.push(FullSet {
            slug: catalog.by_name(&name).map(|c| c.slug.clone()),
            result: recipe.result_type.clone(),
            parts,
            complete,
            name,
        });
    }
    out.sort_by(|a, b| b.complete.cmp(&a.complete).then(a.name.cmp(&b.name)));
    out
}
