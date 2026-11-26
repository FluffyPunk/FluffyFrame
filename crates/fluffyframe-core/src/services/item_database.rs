//! Item database built from DE's public export (original: `services/itemDatabase.ts`,
//! `services/regionNames.ts`, `config/shared/displayName.ts`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::public_export::ExportSet;
use crate::config::constants::PUBLIC_EXPORT_IMAGE_BASE;
use crate::config::game_tables::{self, split_camel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    Warframe,
    Archwing,
    Necramech,
    Primary,
    Secondary,
    Melee,
    ArchGun,
    ArchMelee,
    Amp,
    Companion,
    CompanionWeapon,
    Mod,
    Arcane,
    Relic,
    Resource,
    Component,
    Blueprint,
    Gear,
    Key,
    Drone,
    Endo,
    Other,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Warframe => "Warframe",
            ItemKind::Archwing => "Archwing",
            ItemKind::Necramech => "Necramech",
            ItemKind::Primary => "Primary",
            ItemKind::Secondary => "Secondary",
            ItemKind::Melee => "Melee",
            ItemKind::ArchGun => "Arch-Gun",
            ItemKind::ArchMelee => "Arch-Melee",
            ItemKind::Amp => "Amp",
            ItemKind::Companion => "Companion",
            ItemKind::CompanionWeapon => "Companion Weapon",
            ItemKind::Mod => "Mod",
            ItemKind::Arcane => "Arcane",
            ItemKind::Relic => "Relic",
            ItemKind::Resource => "Resource",
            ItemKind::Component => "Component",
            ItemKind::Blueprint => "Blueprint",
            ItemKind::Gear => "Gear",
            ItemKind::Key => "Key",
            ItemKind::Drone => "Extractor",
            ItemKind::Endo => "Endo",
            ItemKind::Other => "Other",
        }
    }

    /// Warframe-like gear earns 200 mastery per rank and needs 1000·r² affinity.
    pub fn is_frame_like(self) -> bool {
        matches!(
            self,
            ItemKind::Warframe | ItemKind::Archwing | ItemKind::Necramech | ItemKind::Companion
        )
    }

    pub fn is_equipment(self) -> bool {
        matches!(
            self,
            ItemKind::Warframe
                | ItemKind::Archwing
                | ItemKind::Necramech
                | ItemKind::Primary
                | ItemKind::Secondary
                | ItemKind::Melee
                | ItemKind::ArchGun
                | ItemKind::ArchMelee
                | ItemKind::Amp
                | ItemKind::Companion
                | ItemKind::CompanionWeapon
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemInfo {
    pub unique_name: String,
    pub name: String,
    pub description: String,
    pub kind: ItemKind,
    pub product_category: Option<String>,
    pub mastery_req: u32,
    pub ducats: Option<u32>,
    pub texture: Option<String>,
    /// Max mod/arcane rank.
    pub max_rank: Option<u32>,
    pub rarity: Option<String>,
    pub polarity: Option<String>,
    pub mod_type: Option<String>,
    pub compat_name: Option<String>,
    pub masterable: bool,
    /// Max equipment level (30, or 40 for Kuva/Tenet/Coda and Necramechs).
    pub max_level: u32,
    pub vaulted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ingredient {
    pub item_type: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub unique_name: String,
    pub result_type: String,
    pub build_price: u64,
    pub build_time_secs: u64,
    pub num: u32,
    pub consume_on_use: bool,
    pub ingredients: Vec<Ingredient>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RelicEra {
    Lith,
    Meso,
    Neo,
    Axi,
    Requiem,
    Omnia,
    Other,
}

impl RelicEra {
    pub fn from_name(name: &str) -> Self {
        match name.split_whitespace().next().unwrap_or("") {
            "Lith" => RelicEra::Lith,
            "Meso" => RelicEra::Meso,
            "Neo" => RelicEra::Neo,
            "Axi" => RelicEra::Axi,
            "Requiem" => RelicEra::Requiem,
            "Omnia" => RelicEra::Omnia,
            _ => RelicEra::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RelicEra::Lith => "Lith",
            RelicEra::Meso => "Meso",
            RelicEra::Neo => "Neo",
            RelicEra::Axi => "Axi",
            RelicEra::Requiem => "Requiem",
            RelicEra::Omnia => "Omnia",
            RelicEra::Other => "Other",
        }
    }

    pub const ALL: [RelicEra; 6] = [
        RelicEra::Lith,
        RelicEra::Meso,
        RelicEra::Neo,
        RelicEra::Axi,
        RelicEra::Requiem,
        RelicEra::Omnia,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Refinement {
    Intact,
    Exceptional,
    Flawless,
    Radiant,
}

impl Refinement {
    /// DE encodes refinement in the unique name: Bronze/Silver/Gold/Platinum.
    pub fn from_unique_name(unique: &str) -> (Self, &str) {
        for (suffix, r) in [
            ("Bronze", Refinement::Intact),
            ("Silver", Refinement::Exceptional),
            ("Gold", Refinement::Flawless),
            ("Platinum", Refinement::Radiant),
        ] {
            if let Some(base) = unique.strip_suffix(suffix) {
                return (r, base);
            }
        }
        (Refinement::Intact, unique)
    }

    pub fn label(self) -> &'static str {
        match self {
            Refinement::Intact => "Intact",
            Refinement::Exceptional => "Exceptional",
            Refinement::Flawless => "Flawless",
            Refinement::Radiant => "Radiant",
        }
    }

    /// Per-item drop chance for (common, uncommon, rare).
    pub fn chances(self) -> (f64, f64, f64) {
        match self {
            Refinement::Intact => (0.2533, 0.11, 0.02),
            Refinement::Exceptional => (0.2333, 0.13, 0.04),
            Refinement::Flawless => (0.20, 0.17, 0.06),
            Refinement::Radiant => (0.1667, 0.20, 0.10),
        }
    }

    pub const ALL: [Refinement; 4] = [
        Refinement::Intact,
        Refinement::Exceptional,
        Refinement::Flawless,
        Refinement::Radiant,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
}

impl Rarity {
    pub fn parse(s: &str) -> Self {
        match s {
            "RARE" => Rarity::Rare,
            "UNCOMMON" => Rarity::Uncommon,
            _ => Rarity::Common,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
        }
    }
    pub fn chance(self, refinement: Refinement) -> f64 {
        let (c, u, r) = refinement.chances();
        match self {
            Rarity::Common => c,
            Rarity::Uncommon => u,
            Rarity::Rare => r,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelicReward {
    pub unique_name: String,
    pub rarity: Rarity,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelicInfo {
    pub unique_name: String,
    /// "Axi A1"
    pub name: String,
    pub era: RelicEra,
    pub refinement: Refinement,
    /// Unique name without the refinement suffix; equal across refinements.
    pub family: String,
    pub rewards: Vec<RelicReward>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub id: String,
    pub name: String,
    pub system: String,
    pub mission_type: String,
    pub faction: String,
    pub min_level: u32,
    pub max_level: u32,
}

#[derive(Default)]
pub struct ItemDb {
    pub items: HashMap<String, ItemInfo>,
    pub recipes: HashMap<String, Recipe>,
    /// result uniqueName → recipe uniqueName
    pub recipe_by_result: HashMap<String, String>,
    pub relics: HashMap<String, RelicInfo>,
    pub regions: HashMap<String, Region>,
    /// Riven stat tables and weapon dispositions.
    pub rivens: super::riven_data::RivenTables,
    /// Sigil uniqueName → texture. Sigils aren't items here (ExportFlavour isn't
    /// downloaded), but syndicate emblems are sigils.
    sigil_textures: HashMap<String, String>,
}

/// "<ARCHWING> Odonata Prime" → "Odonata Prime".
pub fn sanitize_name(name: &str) -> String {
    let mut out = name.trim();
    while let Some(rest) = out.strip_prefix('<') {
        match rest.find('>') {
            Some(end) => out = rest[end + 1..].trim_start(),
            None => break,
        }
    }
    let out = out.trim();
    // DE ships some names in capitals ("ADRA MUTAGEN"); show them in title case.
    let letters: Vec<char> = out.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 3 && letters.iter().all(|c| c.is_uppercase()) {
        return crate::config::game_tables::title_case(out);
    }
    out.to_string()
}

/// Modular parts that do not earn mastery themselves: only the Zaw strike, kitgun
/// chamber, amp prism, K-Drive deck and MOA/hound model count.
pub fn is_modular_component(unique: &str, name: &str) -> bool {
    const PART_DIRS: [&str; 6] = ["/Balance/", "/Clip/", "/Clips/", "/Handle/", "/Handles/", "/Grip/"];
    if PART_DIRS.iter().any(|d| unique.contains(d)) {
        return true;
    }
    if unique.contains("OperatorAmplifiers") && unique.contains("/Chassis/") {
        return true;
    }
    if unique.contains("PetParts/") {
        return !(name.ends_with(" Moa") || name.ends_with(" Hound"));
    }
    if unique.contains("/HoverboardParts/") {
        return !unique.ends_with("Deck");
    }
    false
}

/// Readable fallback from a Lotus path tail.
pub fn fallback_name(unique: &str) -> String {
    let tail = unique.rsplit('/').next().unwrap_or(unique);
    split_camel(tail)
}

/// `/Lotus/StoreItems/Types/…` → `/Lotus/Types/…`
pub fn store_item_path(path: &str) -> String {
    path.replacen("/StoreItems/", "/", 1)
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}
fn u64_field(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|x| x.as_u64().or_else(|| x.as_f64().map(|f| f as u64)))
}

fn kind_for(file: &str, category: Option<&str>) -> ItemKind {
    match (file, category.unwrap_or("")) {
        (_, "SpecialItems") => ItemKind::Other,
        ("ExportWarframes", "Suits") => ItemKind::Warframe,
        ("ExportWarframes", "SpaceSuits") => ItemKind::Archwing,
        ("ExportWarframes", "MechSuits") => ItemKind::Necramech,
        ("ExportWeapons", "LongGuns") => ItemKind::Primary,
        ("ExportWeapons", "Pistols") => ItemKind::Secondary,
        ("ExportWeapons", "Melee") => ItemKind::Melee,
        ("ExportWeapons", "SpaceGuns") => ItemKind::ArchGun,
        ("ExportWeapons", "SpaceMelee") => ItemKind::ArchMelee,
        ("ExportWeapons", "OperatorAmps") => ItemKind::Amp,
        ("ExportWeapons", "SentinelWeapons") => ItemKind::CompanionWeapon,
        ("ExportSentinels", "Sentinels" | "KubrowPets") => ItemKind::Companion,
        _ => ItemKind::Other,
    }
}

impl ItemDb {
    pub fn build(exports: &ExportSet) -> Self {
        let mut db = ItemDb { rivens: super::riven_data::RivenTables::build(exports), ..Default::default() };

        let textures: HashMap<&str, &str> = exports
            .array("ExportManifest", "Manifest")
            .iter()
            .filter_map(|v| Some((v.get("uniqueName")?.as_str()?, v.get("textureLocation")?.as_str()?)))
            .collect();
        db.sigil_textures = textures
            .iter()
            .filter(|(u, _)| u.starts_with("/Lotus/Upgrades/Skins/Sigils/"))
            .map(|(u, t)| (u.to_string(), t.to_string()))
            .collect();

        let add = |v: &Value, kind: ItemKind, db: &mut ItemDb| {
            let Some(unique) = str_field(v, "uniqueName") else { return };
            let raw_name = str_field(v, "name").unwrap_or_default();
            let name = if raw_name.trim().is_empty() {
                fallback_name(&unique)
            } else {
                sanitize_name(&raw_name)
            };
            let description = match v.get("description") {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Array(lines)) => lines
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            let category = str_field(v, "productCategory");
            let max_rank = match kind {
                ItemKind::Mod => u64_field(v, "fusionLimit").map(|n| n as u32),
                ItemKind::Arcane => v
                    .get("levelStats")
                    .and_then(Value::as_array)
                    .map(|a| a.len().saturating_sub(1) as u32),
                _ => None,
            };
            let masterable = kind.is_equipment()
                && v.get("excludeFromCodex") != Some(&Value::Bool(true))
                && !is_modular_component(&unique, &name);
            let max_level = u64_field(v, "maxLevelCap")
                .map(|n| n as u32)
                .unwrap_or_else(|| {
                    let forty = kind == ItemKind::Necramech
                        || ["Kuva ", "Tenet ", "Coda "].iter().any(|p| name.starts_with(p))
                        || name == "Paracesis";
                    if forty { 40 } else { 30 }
                });
            let info = ItemInfo {
                texture: textures.get(unique.as_str()).map(|t| t.to_string()),
                ducats: u64_field(v, "primeSellingPrice").map(|n| n as u32),
                mastery_req: u64_field(v, "masteryReq").unwrap_or(0) as u32,
                rarity: str_field(v, "rarity"),
                polarity: str_field(v, "polarity"),
                mod_type: str_field(v, "type"),
                compat_name: str_field(v, "compatName"),
                vaulted: v.get("vaulted").and_then(Value::as_bool).unwrap_or(false),
                unique_name: unique.clone(),
                name,
                description,
                kind,
                product_category: category,
                max_rank,
                masterable,
                max_level,
            };
            db.items.insert(unique, info);
        };

        for file in ["ExportWarframes", "ExportWeapons", "ExportSentinels"] {
            for v in exports.array(file, file) {
                let mut kind = kind_for(file, v.get("productCategory").and_then(Value::as_str));
                // DE files amp prisms under "Pistols"; they master as amps.
                if v.get("uniqueName").and_then(Value::as_str).is_some_and(|u| u.contains("/OperatorAmplifiers/")) {
                    kind = ItemKind::Amp;
                }
                add(v, kind, &mut db);
            }
        }
        for v in exports.array("ExportWeapons", "ExportRailjackWeapons") {
            add(v, ItemKind::Other, &mut db);
        }
        for v in exports.array("ExportUpgrades", "ExportUpgrades") {
            add(v, ItemKind::Mod, &mut db);
        }
        // Railjack mods ("avionics") are mods like any other: Mods tab, rank,
        // card art. "Unfused Artifact" entries are the fusion-material templates.
        for v in exports.array("ExportUpgrades", "ExportAvionics") {
            let template = v.get("name").and_then(Value::as_str) == Some("Unfused Artifact");
            add(v, if template { ItemKind::Other } else { ItemKind::Mod }, &mut db);
        }
        for v in exports.array("ExportResources", "ExportResources") {
            let unique = v.get("uniqueName").and_then(Value::as_str).unwrap_or("");
            let kind = if unique.contains("/Recipes/") || unique.contains("/WeaponParts/") {
                ItemKind::Component
            } else {
                ItemKind::Resource
            };
            add(v, kind, &mut db);
        }
        for (file, kind) in [
            ("ExportGear", ItemKind::Gear),
            ("ExportKeys", ItemKind::Key),
            ("ExportDrones", ItemKind::Drone),
            ("ExportFusionBundles", ItemKind::Endo),
        ] {
            for v in exports.array(file, file) {
                add(v, kind, &mut db);
            }
        }

        // Relics and arcanes share one manifest; only relicRewards tells them apart.
        for v in exports.array("ExportRelicArcane", "ExportRelicArcane") {
            if let Some(rewards) = v.get("relicRewards").and_then(Value::as_array) {
                add(v, ItemKind::Relic, &mut db);
                let Some(unique) = str_field(v, "uniqueName") else { continue };
                let (refinement, family) = Refinement::from_unique_name(&unique);
                let full = sanitize_name(&str_field(v, "name").unwrap_or_default());
                let name = full.strip_suffix(" Relic").unwrap_or(&full).to_string();
                let rewards = rewards
                    .iter()
                    .filter_map(|r| {
                        Some(RelicReward {
                            unique_name: store_item_path(r.get("rewardName")?.as_str()?),
                            rarity: Rarity::parse(r.get("rarity").and_then(Value::as_str).unwrap_or("")),
                            count: u64_field(r, "itemCount").unwrap_or(1) as u32,
                        })
                    })
                    .collect();
                db.relics.insert(
                    unique.clone(),
                    RelicInfo {
                        era: RelicEra::from_name(&name),
                        family: family.to_string(),
                        unique_name: unique,
                        name,
                        refinement,
                        rewards,
                    },
                );
            } else {
                add(v, ItemKind::Arcane, &mut db);
            }
        }

        for v in exports.array("ExportRecipes", "ExportRecipes") {
            let (Some(unique), Some(result)) = (str_field(v, "uniqueName"), str_field(v, "resultType")) else {
                continue;
            };
            let ingredients = v
                .get("ingredients")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|i| {
                            Some(Ingredient {
                                item_type: i.get("ItemType")?.as_str()?.to_string(),
                                count: u64_field(i, "ItemCount").unwrap_or(1) as u32,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            let recipe = Recipe {
                unique_name: unique.clone(),
                result_type: result.clone(),
                build_price: u64_field(v, "buildPrice").unwrap_or(0),
                build_time_secs: u64_field(v, "buildTime").unwrap_or(0),
                num: u64_field(v, "num").unwrap_or(1) as u32,
                consume_on_use: v.get("consumeOnUse").and_then(Value::as_bool).unwrap_or(true),
                ingredients,
            };
            db.recipe_by_result.entry(result.clone()).or_insert_with(|| unique.clone());
            db.recipes.insert(unique.clone(), recipe);

            // Blueprints have no name in the export: derive it from the result.
            if !db.items.contains_key(&unique) {
                let result_name = db
                    .items
                    .get(&result)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| fallback_name(&result).replace(" Component", ""));
                let info = ItemInfo {
                    unique_name: unique.clone(),
                    name: format!("{result_name} Blueprint"),
                    description: String::new(),
                    kind: ItemKind::Blueprint,
                    product_category: None,
                    mastery_req: 0,
                    ducats: u64_field(v, "primeSellingPrice").map(|n| n as u32),
                    texture: textures
                        .get(unique.as_str())
                        .or_else(|| textures.get(result.as_str()))
                        .map(|t| t.to_string()),
                    max_rank: None,
                    rarity: None,
                    polarity: None,
                    mod_type: None,
                    compat_name: None,
                    masterable: false,
                    max_level: 0,
                    vaulted: false,
                };
                db.items.insert(unique, info);
            }
        }

        for v in exports.array("ExportRegions", "ExportRegions") {
            let Some(id) = str_field(v, "uniqueName") else { continue };
            let mt = v
                .get("missionIndex")
                .and_then(Value::as_i64)
                .and_then(game_tables::mission_index_to_mt);
            db.regions.insert(
                id.clone(),
                Region {
                    name: str_field(v, "name").unwrap_or_else(|| id.clone()),
                    system: str_field(v, "systemName").unwrap_or_default(),
                    mission_type: mt.map(game_tables::mission_type_label).unwrap_or_default(),
                    faction: game_tables::faction_index_label(
                        v.get("factionIndex").and_then(Value::as_i64).unwrap_or(-1),
                    )
                    .to_string(),
                    min_level: u64_field(v, "minEnemyLevel").unwrap_or(0) as u32,
                    max_level: u64_field(v, "maxEnemyLevel").unwrap_or(0) as u32,
                    id,
                },
            );
        }

        log::info!(
            "item db: {} items, {} recipes, {} relics, {} nodes",
            db.items.len(),
            db.recipes.len(),
            db.relics.len(),
            db.regions.len()
        );
        db
    }

    pub fn get(&self, unique: &str) -> Option<&ItemInfo> {
        self.items.get(unique)
    }

    pub fn name(&self, unique: &str) -> String {
        self.items
            .get(unique)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| fallback_name(unique))
    }

    /// Framed wiki card for mods/arcanes (and a few overrides), else DE's texture.
    pub fn image_url(&self, unique: &str) -> Option<String> {
        let info = self.items.get(unique);
        let card = matches!(info.map(|i| i.kind), Some(ItemKind::Mod | ItemKind::Arcane));
        super::wiki_art::card_art_url(unique, info.map(|i| i.name.as_str()).unwrap_or(""), card).or_else(|| {
            info.and_then(|i| i.texture.as_deref()).map(|t| format!("{PUBLIC_EXPORT_IMAGE_BASE}{t}"))
        })
    }

    /// Emblem image for a syndicate at `rank` (see [`game_tables::syndicate_emblem`]).
    pub fn syndicate_emblem_url(&self, tag: &str, rank: i64) -> Option<String> {
        let sigil = crate::config::game_tables::syndicate_emblem(tag, rank)?;
        self.sigil_textures.get(&sigil).map(|t| format!("{PUBLIC_EXPORT_IMAGE_BASE}{t}"))
    }

    pub fn recipe_for_result(&self, result: &str) -> Option<&Recipe> {
        self.recipe_by_result.get(result).and_then(|r| self.recipes.get(r))
    }

    /// "Galatea (Neptune)"
    pub fn node_label(&self, node: &str) -> String {
        match self.regions.get(node) {
            Some(r) if !r.system.is_empty() => format!("{} ({})", r.name, r.system),
            Some(r) => r.name.clone(),
            // Railjack nodes are missing from ExportRegions.
            None => match node.strip_prefix("CrewBattleNode") {
                Some(n) => format!("Railjack Node {n}"),
                None => node.to_string(),
            },
        }
    }

    /// Case-insensitive substring search, prefix matches first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&ItemInfo> {
        let q = query.trim().to_lowercase();
        if q.len() < 2 {
            return Vec::new();
        }
        let mut hits: Vec<&ItemInfo> = self
            .items
            .values()
            .filter(|i| i.kind != ItemKind::Other && i.name.to_lowercase().contains(&q))
            .collect();
        hits.sort_by_key(|i| (!i.name.to_lowercase().starts_with(&q), i.name.len(), i.name.clone()));
        hits.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);
        hits.truncate(limit);
        hits
    }

    /// Relic families (intact unique names) that contain `unique` as a reward.
    pub fn relics_dropping(&self, unique: &str) -> Vec<(&RelicInfo, Rarity)> {
        let mut out: Vec<(&RelicInfo, Rarity)> = self
            .relics
            .values()
            .filter(|r| r.refinement == Refinement::Intact)
            .filter_map(|r| {
                r.rewards
                    .iter()
                    .find(|w| w.unique_name == unique)
                    .map(|w| (r, w.rarity))
            })
            .collect();
        out.sort_by(|a, b| (a.0.era, &a.0.name).cmp(&(b.0.era, &b.0.name)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(sanitize_name("<ARCHWING> Odonata Prime"), "Odonata Prime");
        assert_eq!(sanitize_name("ADRA MUTAGEN"), "Adra Mutagen");
        assert_eq!(sanitize_name("MK1-Braton"), "MK1-Braton");
        assert_eq!(fallback_name("/Lotus/Types/Items/MiscItems/OrokinCell"), "Orokin Cell");
        assert_eq!(
            store_item_path("/Lotus/StoreItems/Types/Recipes/X"),
            "/Lotus/Types/Recipes/X"
        );
    }

    #[test]
    fn modular_components() {
        assert!(is_modular_component("/Lotus/Weapons/Ostron/Melee/ModularMelee01/Handle/HandleOne", "Peye"));
        assert!(!is_modular_component("/Lotus/Weapons/Ostron/Melee/ModularMelee01/Tip/TipOne", "Balla"));
        assert!(is_modular_component("/Lotus/Weapons/Corpus/OperatorAmplifiers/Set1/Chassis/CorpAmpSet1ChassisPartA", "Exard Scaffold"));
        assert!(!is_modular_component("/Lotus/Weapons/Corpus/OperatorAmplifiers/Set1/Barrel/CorpAmpSet1BarrelPartA", "Cantic Prism"));
        assert!(is_modular_component("/Lotus/Types/Friendly/Pets/MoaPets/MoaPetParts/MoaPetEngineLehan", "Lehan Core"));
        assert!(!is_modular_component("/Lotus/Types/Friendly/Pets/MoaPets/MoaPetParts/MoaPetHeadOloro", "Oloro Moa"));
    }

    #[test]
    fn refinement_suffix() {
        let (r, base) = Refinement::from_unique_name("/Lotus/Types/Game/Projections/T4VoidProjectionXakuPrimeDGold");
        assert_eq!(r, Refinement::Flawless);
        assert!(base.ends_with("XakuPrimeD"));
    }
}
