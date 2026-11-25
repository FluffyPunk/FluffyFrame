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
    /// Copies that can actually be traded: built weapons are account-bound,
    /// so a weapon part counts only as sets of its tradable parts.
    pub tradable_owned: u64,
}

#[derive(Debug, Clone)]
pub struct FullSet {
    pub name: String,
    pub result: String,
    pub slug: Option<String>,
    pub parts: Vec<SetPart>,
    /// Sets the owned parts can craft (built weapons count).
    pub complete: u64,
    /// Sets that can be sold as a set (tradable parts only).
    pub tradable_complete: u64,
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
        let (complete, tradable_complete) = counts_of(&parts);
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
            tradable_complete,
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
    memo: &mut HashMap<String, (u64, u64)>,
) -> Vec<SetPart> {
    let Some(recipe) = db.recipes.get(recipe_id) else { return Vec::new() };
    let main_owned = counts.get(recipe.unique_name.as_str()).copied().unwrap_or(0);
    let main_tradable = catalog.by_game_ref(&recipe.unique_name).is_some();
    let mut parts = vec![SetPart {
        name: db.name(&recipe.unique_name),
        owned: main_owned,
        needed: 1,
        tradable: main_tradable,
        tradable_owned: if main_tradable { main_owned } else { 0 },
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
        let (name, owned, tradable, tradable_owned) = if weapon {
            let (sets, tradable_sets) = blueprint.map_or((0, 0), |b| complete_sets(b, db, catalog, counts, memo));
            let name = format!("{} Set", db.name(&ing.item_type));
            let tradable = catalog.by_name(&name).is_some() || catalog.by_game_ref(&ing.item_type).is_some();
            // Built weapons craft but never trade.
            (name, built + sets, tradable, tradable_sets)
        } else {
            let bp_listed = blueprint.filter(|b| catalog.by_game_ref(b).is_some());
            // Name the part the way the market lists it (blueprint form when tradable).
            let name = bp_listed.map(|b| db.name(b)).unwrap_or_else(|| db.name(&ing.item_type));
            let tradable = bp_listed.is_some() || catalog.by_game_ref(&ing.item_type).is_some();
            let owned = built + blueprint.and_then(|b| counts.get(b.as_str())).copied().unwrap_or(0);
            (name, owned, tradable, if tradable { owned } else { 0 })
        };
        parts.push(SetPart { name, owned, needed: ing.count as u64, tradable, tradable_owned });
    }
    parts
}

/// (craftable, tradable) set counts for parts.
fn counts_of(parts: &[SetPart]) -> (u64, u64) {
    let min = |f: fn(&SetPart) -> u64| parts.iter().map(|p| f(p) / p.needed.max(1)).min().unwrap_or(0);
    (min(|p| p.owned), min(|p| p.tradable_owned))
}

/// How many full sets of `recipe_id` the owned parts make, (craftable, tradable)
/// (memoised; a recipe that would contain itself counts as none).
fn complete_sets(
    recipe_id: &str,
    db: &ItemDb,
    catalog: &Catalog,
    counts: &HashMap<&str, u64>,
    memo: &mut HashMap<String, (u64, u64)>,
) -> (u64, u64) {
    if let Some(n) = memo.get(recipe_id) {
        return *n;
    }
    memo.insert(recipe_id.to_string(), (0, 0));
    let parts = set_parts(recipe_id, db, catalog, counts, memo);
    let n = if parts.len() < 2 { (0, 0) } else { counts_of(&parts) };
    memo.insert(recipe_id.to_string(), n);
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::inventory::{OwnedEquipment, OwnedItem};
    use crate::services::item_database::{Ingredient, ItemInfo, Recipe};
    use crate::services::wfm_catalog::CatalogItem;

    fn info(unique: &str, name: &str, kind: ItemKind) -> ItemInfo {
        ItemInfo {
            unique_name: unique.into(),
            name: name.into(),
            description: String::new(),
            kind,
            product_category: None,
            mastery_req: 0,
            ducats: None,
            texture: None,
            max_rank: None,
            rarity: None,
            polarity: None,
            mod_type: None,
            compat_name: None,
            masterable: true,
            max_level: 30,
            vaulted: false,
        }
    }

    fn listed(game_ref: Option<&str>, name: &str) -> CatalogItem {
        CatalogItem {
            id: name.into(),
            slug: name.to_lowercase().replace(' ', "_"),
            name: name.into(),
            game_ref: game_ref.map(str::to_string),
            ducats: None,
            max_rank: None,
            tags: vec![],
            thumb: None,
        }
    }

    /// Aklex Prime = Aklex blueprint + link + two Lex Primes. With two *built*
    /// Lex Primes the set can be crafted, but none of it can be sold as a set:
    /// built weapons are account-bound.
    #[test]
    fn built_weapons_craft_but_do_not_trade() {
        let mut db = ItemDb::default();
        for (u, n, k) in [
            ("/Aklex", "Aklex Prime", ItemKind::Secondary),
            ("/Lex", "Lex Prime", ItemKind::Secondary),
            ("/AklexBp", "Aklex Prime Blueprint", ItemKind::Blueprint),
            ("/AklexLink", "Aklex Prime Link", ItemKind::Component),
            ("/LexBp", "Lex Prime Blueprint", ItemKind::Blueprint),
            ("/LexBarrel", "Lex Prime Barrel", ItemKind::Component),
        ] {
            db.items.insert(u.into(), info(u, n, k));
        }
        let recipe = |id: &str, result: &str, ings: &[(&str, u32)]| Recipe {
            unique_name: id.into(),
            result_type: result.into(),
            build_price: 0,
            build_time_secs: 0,
            num: 1,
            consume_on_use: true,
            ingredients: ings.iter().map(|(t, c)| Ingredient { item_type: (*t).into(), count: *c }).collect(),
        };
        // DE lists the Lex Prime ingredient twice; the set shows it once, ×2.
        db.recipes.insert("/AklexBp".into(), recipe("/AklexBp", "/Aklex", &[("/Lex", 1), ("/Lex", 1), ("/AklexLink", 1)]));
        db.recipes.insert("/LexBp".into(), recipe("/LexBp", "/Lex", &[("/LexBarrel", 1)]));
        db.recipe_by_result.insert("/Aklex".into(), "/AklexBp".into());
        db.recipe_by_result.insert("/Lex".into(), "/LexBp".into());
        let catalog = Catalog::from_items(vec![
            listed(Some("/AklexBp"), "Aklex Prime Blueprint"),
            listed(Some("/AklexLink"), "Aklex Prime Link"),
            listed(Some("/LexBp"), "Lex Prime Blueprint"),
            listed(Some("/LexBarrel"), "Lex Prime Barrel"),
            listed(None, "Aklex Prime Set"),
            listed(None, "Lex Prime Set"),
        ]);
        let mut inv = Inventory::default();
        for u in ["/AklexBp", "/AklexLink"] {
            inv.items.push(OwnedItem { unique_name: u.into(), count: 1, rank: None, collection: "Recipes".into() });
        }
        for _ in 0..2 {
            inv.equipment.push(OwnedEquipment {
                unique_name: "/Lex".into(),
                collection: "Pistols".into(),
                xp: 0,
                item_id: None,
                favorite: false,
                shard_slots: vec![],
            });
        }
        let sets = compute(&inv, &db, &catalog, false);
        let aklex = sets.iter().find(|s| s.name == "Aklex Prime Set").expect("craftable set listed");
        let lex = aklex.parts.iter().find(|p| p.name == "Lex Prime Set").unwrap();
        assert_eq!((lex.owned, lex.needed, lex.tradable_owned), (2, 2, 0));
        assert_eq!(aklex.parts.len(), 3, "the two Lex Prime entries are merged");
        assert_eq!((aklex.complete, aklex.tradable_complete), (1, 0));
    }
}
