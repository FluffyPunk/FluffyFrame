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
    /// Listed on warframe.market (as the part, its blueprint, or its set).
    pub tradable: bool,
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
    let mut memo = HashMap::new();
    let mut out = Vec::new();
    for recipe in db.recipes.values() {
        let Some(result) = db.get(&recipe.result_type) else { continue };
        if !result.kind.is_equipment() {
            continue;
        }
        let mut parts = set_parts(&recipe.unique_name, db, catalog, &counts, &mut memo);
        // A set trades when its main blueprint does, or when every other part
        // does (Purgator 1: blueprint untradable, barrel/receiver/stock tradable);
        // then the market set is just those parts.
        if !parts[0].tradable {
            if parts.len() < 3 || !parts[1..].iter().all(|p| p.tradable) {
                continue;
            }
            parts.remove(0);
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

/// The main blueprint plus each crafted part, repeats merged (Aklex lists Lex
/// Prime twice). A crafted weapon used as a part (Lex Prime in Aklex) is its own
/// set: owned built, or as complete sets of its parts.
fn set_parts(
    recipe_id: &str,
    db: &ItemDb,
    catalog: &Catalog,
    counts: &HashMap<&str, u64>,
    memo: &mut HashMap<String, u64>,
) -> Vec<SetPart> {
    let Some(recipe) = db.recipes.get(recipe_id) else { return Vec::new() };
    let mut parts = vec![SetPart {
        name: db.name(&recipe.unique_name),
        owned: counts.get(recipe.unique_name.as_str()).copied().unwrap_or(0),
        needed: 1,
        tradable: catalog.by_game_ref(&recipe.unique_name).is_some(),
    }];
    let mut seen: Vec<&str> = Vec::new();
    for ing in &recipe.ingredients {
        let Some(info) = db.get(&ing.item_type) else { continue };
        let weapon = info.kind.is_equipment();
        if !(weapon || matches!(info.kind, ItemKind::Component)) {
            continue;
        }
        if let Some(i) = seen.iter().position(|t| *t == ing.item_type) {
            parts[i + 1].needed += ing.count as u64;
            continue;
        }
        seen.push(&ing.item_type);
        let built = counts.get(ing.item_type.as_str()).copied().unwrap_or(0);
        let blueprint = db.recipe_by_result.get(&ing.item_type);
        let (name, owned, tradable) = if weapon {
            let sets = blueprint.map_or(0, |b| complete_sets(b, db, catalog, counts, memo));
            let name = format!("{} Set", db.name(&ing.item_type));
            let tradable = catalog.by_name(&name).is_some() || catalog.by_game_ref(&ing.item_type).is_some();
            (name, built + sets, tradable)
        } else {
            let bp_listed = blueprint.filter(|b| catalog.by_game_ref(b).is_some());
            // Name the part the way the market lists it (blueprint form when tradable).
            let name = bp_listed.map(|b| db.name(b)).unwrap_or_else(|| db.name(&ing.item_type));
            let tradable = bp_listed.is_some() || catalog.by_game_ref(&ing.item_type).is_some();
            (name, built + blueprint.and_then(|b| counts.get(b.as_str())).copied().unwrap_or(0), tradable)
        };
        parts.push(SetPart { name, owned, needed: ing.count as u64, tradable });
    }
    parts
}

/// How many full sets of `recipe_id` the owned parts make (memoised; a recipe
/// that would contain itself counts as none).
fn complete_sets(recipe_id: &str, db: &ItemDb, catalog: &Catalog, counts: &HashMap<&str, u64>, memo: &mut HashMap<String, u64>) -> u64 {
    if let Some(n) = memo.get(recipe_id) {
        return *n;
    }
    memo.insert(recipe_id.to_string(), 0);
    let parts = set_parts(recipe_id, db, catalog, counts, memo);
    let n = if parts.len() < 2 { 0 } else { parts.iter().map(|p| p.owned / p.needed.max(1)).min().unwrap_or(0) };
    memo.insert(recipe_id.to_string(), n);
    n
}
