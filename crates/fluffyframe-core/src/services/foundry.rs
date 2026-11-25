//! Crafting requirements for owned blueprints (original: `src/lib/craftingTree.ts`,
//! `src/lib/inventory/foundryResources.ts`, `config/shared/foundryPending.ts`).

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::inventory::Inventory;
use super::item_database::ItemDb;

#[derive(Debug, Clone)]
pub struct IngredientRow {
    pub unique_name: String,
    pub name: String,
    pub need: u64,
    pub have: u64,
    /// A blueprint for this ingredient exists (it is itself craftable).
    pub craftable: bool,
}

#[derive(Debug, Clone)]
pub struct FoundryRow {
    pub recipe: String,
    pub name: String,
    pub result_name: String,
    pub owned_blueprints: u64,
    /// How many times it can be built now (ingredients and blueprints permitting).
    pub buildable: u64,
    pub build_price: u64,
    pub build_time_secs: u64,
    pub ingredients: Vec<IngredientRow>,
}

#[derive(Debug, Clone)]
pub struct PendingBuild {
    pub name: String,
    pub completes_at: Option<DateTime<Utc>>,
}

pub fn ingredient_rows(recipe: &super::item_database::Recipe, db: &ItemDb, counts: &HashMap<&str, u64>) -> Vec<IngredientRow> {
    recipe
        .ingredients
        .iter()
        .map(|i| IngredientRow {
            name: db.name(&i.item_type),
            need: i.count as u64,
            have: counts.get(i.item_type.as_str()).copied().unwrap_or(0),
            craftable: db.recipe_for_result(&i.item_type).is_some(),
            unique_name: i.item_type.clone(),
        })
        .collect()
}

pub fn rows(inv: &Inventory, db: &ItemDb) -> Vec<FoundryRow> {
    let counts = inv.count_map();
    let mut out: Vec<FoundryRow> = inv
        .items
        .iter()
        .filter(|i| i.collection == "Recipes")
        .filter_map(|item| {
            let recipe = db.recipes.get(&item.unique_name)?;
            let ingredients = ingredient_rows(recipe, db, &counts);
            let by_ingredients = ingredients
                .iter()
                .map(|i| i.have.checked_div(i.need).unwrap_or(u64::MAX))
                .min()
                .unwrap_or(u64::MAX);
            Some(FoundryRow {
                recipe: recipe.unique_name.clone(),
                name: db.name(&recipe.unique_name),
                result_name: db.name(&recipe.result_type),
                owned_blueprints: item.count,
                buildable: by_ingredients.min(item.count),
                build_price: recipe.build_price,
                build_time_secs: recipe.build_time_secs,
                ingredients,
            })
        })
        .collect();
    out.sort_by(|a, b| b.buildable.min(1).cmp(&a.buildable.min(1)).then(a.name.cmp(&b.name)));
    out
}

pub fn pending(inv: &Inventory, db: &ItemDb) -> Vec<PendingBuild> {
    let mut out: Vec<PendingBuild> = inv
        .pending_recipes
        .iter()
        .map(|p| PendingBuild {
            name: db
                .recipes
                .get(&p.item_type)
                .map(|r| db.name(&r.result_type))
                .unwrap_or_else(|| db.name(&p.item_type)),
            completes_at: p.completes_at,
        })
        .collect();
    out.sort_by_key(|p| p.completes_at);
    out
}
