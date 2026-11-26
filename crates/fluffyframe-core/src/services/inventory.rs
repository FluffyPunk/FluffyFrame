//! Inventory snapshot model and parser (original: `config/shared/inventoryPayload.ts`,
//! `src/lib/inventory.ts`, `src/lib/inventory/*`).
//!
//! [`Inventory::parse`] turns DE's raw `inventory.php` JSON into typed collections.
//! [`classify`] then groups owned items into the app's category tabs using the
//! item database and the warframe.market catalog.

use std::collections::HashMap;

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::item_database::{ItemDb, ItemKind};
use super::rivens::{self, OwnedRiven};
use super::wfm_stats_price::Variant;
use super::wfm_catalog::Catalog;
use crate::config::settings::InventorySource;

/// Collections holding built gear.
pub const EQUIPMENT_COLLECTIONS: &[&str] = &[
    "Suits",
    "LongGuns",
    "Pistols",
    "Melee",
    "SpaceSuits",
    "SpaceGuns",
    "SpaceMelee",
    "MechSuits",
    "Sentinels",
    "SentinelWeapons",
    "KubrowPets",
    "MoaPets",
    "OperatorAmps",
    "Hoverboards",
];

/// Stackable collections, with how each entry's count is stored.
const STACK_COLLECTIONS: &[&str] = &[
    "MiscItems",
    "Recipes",
    "RawUpgrades",
    "FusionTreasures",
    "LevelKeys",
    "Consumables",
];

const ENVELOPE_KEYS: &[&str] = &["InventoryJson", "inventoryJson", "inventory_json", "payload", "data"];
const SHAPE_KEYS: &[&str] = &["Suits", "RawUpgrades", "Upgrades", "Arcanes", "LevelKeys", "MiscItems"];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Currencies {
    pub platinum: u64,
    pub credits: u64,
    pub endo: u64,
    pub ducats: u64,
    pub aya: u64,
    pub vitus: u64,
    pub kuva: u64,
    pub trades_remaining: Option<u64>,
}

/// Currencies DE stores as `MiscItems` stacks.
pub const DUCATS_ITEM: &str = "/Lotus/Types/Items/MiscItems/PrimeBucks";
pub const AYA_ITEM: &str = "/Lotus/Types/Items/MiscItems/SchismKey";
pub const VITUS_ITEM: &str = "/Lotus/Types/Items/MiscItems/Elitium";
pub const KUVA_ITEM: &str = "/Lotus/Types/Items/MiscItems/Kuva";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedItem {
    pub unique_name: String,
    pub count: u64,
    /// Mod/arcane rank; `None` for unranked stacks.
    pub rank: Option<u32>,
    pub collection: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedEquipment {
    pub unique_name: String,
    pub collection: String,
    pub xp: u64,
    pub item_id: Option<String>,
    pub favorite: bool,
    /// Archon shard sockets in payload order; `None` is an empty socket that DE
    /// still sends in position (Warframes only).
    pub shard_slots: Vec<Option<ArchonShard>>,
}

/// Archon shard hue (original: `lib/inventory/archonShards.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardColor {
    Crimson,
    Amber,
    Azure,
    Emerald,
    Topaz,
    Violet,
}

impl ShardColor {
    /// Socketed shards record their hue as `ACC_<hue>`, `_MYTHIC` = tauforged.
    fn from_acc(acc: &str) -> Option<Self> {
        Some(match acc {
            "ACC_RED" => Self::Crimson,
            "ACC_YELLOW" => Self::Amber,
            "ACC_BLUE" => Self::Azure,
            "ACC_GREEN" => Self::Emerald,
            "ACC_ORANGE" => Self::Topaz,
            "ACC_PURPLE" => Self::Violet,
            _ => return None,
        })
    }

    /// ExportMiscItems uniqueName of the loose shard (for its icon).
    pub fn item_unique(self, tauforged: bool) -> String {
        let leaf = match self {
            Self::Crimson => "ArchonCrystalAmar",
            Self::Amber => "ArchonCrystalNira",
            Self::Azure => "ArchonCrystalBoreal",
            Self::Emerald => "ArchonCrystalGreen",
            Self::Topaz => "ArchonCrystalOrange",
            Self::Violet => "ArchonCrystalViolet",
        };
        format!("/Lotus/Types/Gameplay/NarmerSorties/{leaf}{}", if tauforged { "Mythic" } else { "" })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Crimson => "Crimson",
            Self::Amber => "Amber",
            Self::Azure => "Azure",
            Self::Emerald => "Emerald",
            Self::Topaz => "Topaz",
            Self::Violet => "Violet",
        }
    }
}

/// Sockets a Warframe has; DE only sends up to the highest used one.
pub const ARCHON_SHARD_SLOTS: usize = 5;

/// A filled shard socket. `color` is None when DE omitted or renamed the hue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchonShard {
    pub color: Option<ShardColor>,
    pub tauforged: bool,
    /// `/Lotus/Upgrades/Invigorations/ArchonCrystalUpgrades/ArchonCrystalUpgrade…`
    pub upgrade: String,
}

impl ArchonShard {
    /// Stat the shard grants, from its upgrade path ("…WarframeAbilityStrengthMythic"
    /// → "Ability Strength"); original: `archonShardUpgradeLabel`.
    pub fn stat_label(&self) -> String {
        let leaf = self.upgrade.rsplit('/').next().unwrap_or("");
        let leaf = leaf.strip_prefix("ArchonCrystalUpgrade").unwrap_or(leaf);
        let leaf = leaf.strip_suffix("Mythic").unwrap_or(leaf);
        let chars: Vec<char> = leaf.chars().collect();
        let mut out = String::with_capacity(leaf.len() + 8);
        for (i, &c) in chars.iter().enumerate() {
            if i > 0 && c.is_ascii_uppercase() {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
                // "fooBar" | "HPRegen" → "HP Regen"
                if prev.is_ascii_lowercase() || prev.is_ascii_digit() || (prev.is_ascii_uppercase() && next_lower) {
                    out.push(' ');
                }
            }
            out.push(c);
        }
        out.strip_prefix("Warframe ").unwrap_or(&out).trim().to_string()
    }
}

fn parse_shards(entry: &Value) -> Vec<Option<ArchonShard>> {
    arr(entry, "ArchonCrystalUpgrades")
        .iter()
        .map(|s| {
            let upgrade = s.get("UpgradeType").and_then(Value::as_str).filter(|u| !u.is_empty())?;
            let acc = s.get("Color").and_then(Value::as_str).unwrap_or("");
            let base = acc.strip_suffix("_MYTHIC");
            Some(ArchonShard {
                color: ShardColor::from_acc(base.unwrap_or(acc)),
                tauforged: base.is_some(),
                upgrade: upgrade.to_string(),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRecipe {
    pub item_type: String,
    pub completes_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Affiliation {
    pub tag: String,
    pub standing: i64,
    pub title: i64,
}

/// A star chart node the account has played.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProgress {
    pub tag: String,
    pub completes: u64,
    pub steel_path: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub source: Option<InventorySource>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub mastery_rank: u32,
    pub currencies: Currencies,
    pub items: Vec<OwnedItem>,
    pub equipment: Vec<OwnedEquipment>,
    /// Lifetime XP per item (mastery).
    pub xp_info: HashMap<String, u64>,
    pub rivens: Vec<OwnedRiven>,
    pub pending_recipes: Vec<PendingRecipe>,
    #[serde(default)]
    pub nodes: Vec<NodeProgress>,
    /// Intrinsic skill (`LPS_*`: railjack, `LPS_DRIFT_*`: drifter) → rank.
    #[serde(default)]
    pub intrinsics: HashMap<String, u64>,
    pub affiliations: Vec<Affiliation>,
    /// Standing left today, shared by the aligned syndicates.
    pub daily_standing: Option<i64>,
    /// Standing left today for syndicates with their own cap (open worlds,
    /// Simaris, Conclave), by syndicate tag.
    #[serde(default)]
    pub daily_standing_by_syndicate: HashMap<String, i64>,
}

/// True when `value` looks like an inventory object.
pub fn has_inventory_shape(value: &Value) -> bool {
    SHAPE_KEYS.iter().any(|k| value.get(k).is_some_and(Value::is_array))
}

/// Unwraps helper/export envelopes (`{"InventoryJson": "<json string>"}` etc.).
pub fn unwrap_payload(mut value: Value) -> Option<Value> {
    for _ in 0..8 {
        if has_inventory_shape(&value) {
            return Some(value);
        }
        let next = ENVELOPE_KEYS.iter().find_map(|k| value.get(*k).cloned())?;
        value = match next {
            Value::String(s) => serde_json::from_str(&s).ok()?,
            other => other,
        };
    }
    None
}

fn num(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Value::String(s) => s.parse().ok(),
        Value::Object(o) => o.get("$numberLong").and_then(num),
        _ => None,
    }
}

fn field_u64(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(num)
}

/// DE dates: `{"$date": {"$numberLong": "1700000000000"}}`.
pub fn de_date(v: &Value) -> Option<DateTime<Utc>> {
    let ms = v.get("$date").and_then(num).or_else(|| num(v))?;
    Utc.timestamp_millis_opt(ms as i64).single()
}

fn oid(v: &Value) -> Option<String> {
    v.get("$oid").and_then(Value::as_str).map(str::to_string)
}

fn arr<'a>(root: &'a Value, key: &str) -> &'a [Value] {
    root.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn upgrade_rank(entry: &Value) -> Option<u32> {
    let fp = entry.get("UpgradeFingerprint")?;
    let parsed: Value = match fp {
        Value::String(s) => serde_json::from_str(s).ok()?,
        other => other.clone(),
    };
    parsed.get("lvl").and_then(num).map(|n| n as u32)
}

impl Inventory {
    pub fn parse(raw: Value) -> crate::Result<Self> {
        let root = unwrap_payload(raw).ok_or_else(|| {
            anyhow::anyhow!("this file is not an inventory export (no Suits/MiscItems/Upgrades)")
        })?;
        let mut inv = Inventory {
            mastery_rank: field_u64(&root, "PlayerLevel").unwrap_or(0) as u32,
            ..Default::default()
        };
        inv.currencies = Currencies {
            platinum: field_u64(&root, "PremiumCredits").unwrap_or(0),
            credits: field_u64(&root, "RegularCredits").unwrap_or(0),
            endo: field_u64(&root, "FusionPoints").unwrap_or(0),
            trades_remaining: field_u64(&root, "TradesRemaining"),
            ..Default::default()
        };
        inv.daily_standing = root.get("DailyAffiliation").and_then(Value::as_i64);
        if let Some(obj) = root.as_object() {
            inv.daily_standing_by_syndicate = obj
                .iter()
                .filter_map(|(k, v)| {
                    let tag = crate::config::game_tables::daily_cap_syndicate(k.strip_prefix("DailyAffiliation")?)?;
                    Some((tag.to_string(), v.as_i64()?))
                })
                .collect();
        }

        // Stacks, merged per (item, rank).
        let mut stacks: HashMap<(String, Option<u32>), OwnedItem> = HashMap::new();
        let mut push = |unique: &str, count: u64, rank: Option<u32>, collection: &str| {
            if count == 0 || unique.is_empty() {
                return;
            }
            stacks
                .entry((unique.to_string(), rank))
                .and_modify(|e| e.count += count)
                .or_insert_with(|| OwnedItem {
                    unique_name: unique.to_string(),
                    count,
                    rank,
                    collection: collection.to_string(),
                });
        };
        for collection in STACK_COLLECTIONS {
            for entry in arr(&root, collection) {
                let Some(t) = entry.get("ItemType").and_then(Value::as_str) else { continue };
                let count = field_u64(entry, "ItemCount").unwrap_or(1);
                // Unranked mods and arcanes in RawUpgrades are rank 0.
                let rank = (*collection == "RawUpgrades").then_some(0);
                push(t, count, rank, collection);
            }
        }
        // Ranked upgrades are one instance each; rivens go to their own list.
        for entry in arr(&root, "Upgrades") {
            let Some(t) = entry.get("ItemType").and_then(Value::as_str) else { continue };
            if rivens::is_riven_type(t) {
                if let Some(riven) = rivens::parse_owned(entry) {
                    inv.rivens.push(riven);
                }
                continue;
            }
            push(t, 1, Some(upgrade_rank(entry).unwrap_or(0)), "Upgrades");
        }
        inv.items = stacks.into_values().collect();
        // Ducats, Aya, Vitus and Kuva are inventory stacks, not top-level fields.
        let stack = |unique: &str| inv.items.iter().filter(|i| i.unique_name == unique).map(|i| i.count).sum::<u64>();
        inv.currencies.ducats = stack(DUCATS_ITEM);
        inv.currencies.aya = stack(AYA_ITEM);
        inv.currencies.vitus = stack(VITUS_ITEM);
        inv.currencies.kuva = stack(KUVA_ITEM);
        inv.items.sort_by(|a, b| a.unique_name.cmp(&b.unique_name).then(a.rank.cmp(&b.rank)));

        for collection in EQUIPMENT_COLLECTIONS {
            for entry in arr(&root, collection) {
                let Some(t) = entry.get("ItemType").and_then(Value::as_str) else { continue };
                inv.equipment.push(OwnedEquipment {
                    unique_name: t.to_string(),
                    collection: collection.to_string(),
                    xp: field_u64(entry, "XP").unwrap_or(0),
                    item_id: entry.get("ItemId").and_then(oid),
                    favorite: entry.get("Favorite").and_then(Value::as_bool).unwrap_or(false),
                    shard_slots: parse_shards(entry),
                });
            }
        }

        for entry in arr(&root, "XPInfo") {
            if let (Some(t), Some(xp)) = (entry.get("ItemType").and_then(Value::as_str), field_u64(entry, "XP")) {
                inv.xp_info.insert(t.to_string(), xp);
            }
        }

        for entry in arr(&root, "Missions") {
            if let Some(tag) = entry.get("Tag").and_then(Value::as_str) {
                inv.nodes.push(NodeProgress {
                    tag: tag.to_string(),
                    completes: field_u64(entry, "Completes").unwrap_or(0),
                    steel_path: field_u64(entry, "Tier") == Some(1),
                });
            }
        }
        // LPS_* are intrinsic ranks (LPP_* are unspent point pools).
        if let Some(skills) = root.get("PlayerSkills").and_then(Value::as_object) {
            inv.intrinsics = skills
                .iter()
                .filter(|(k, _)| k.starts_with("LPS_"))
                .filter_map(|(k, v)| Some((k.clone(), v.as_u64()?.min(MAX_INTRINSIC_RANK))))
                .collect();
        }

        for entry in arr(&root, "PendingRecipes") {
            if let Some(t) = entry.get("ItemType").and_then(Value::as_str) {
                inv.pending_recipes.push(PendingRecipe {
                    item_type: t.to_string(),
                    completes_at: entry.get("CompletionDate").and_then(de_date),
                });
            }
        }

        for entry in arr(&root, "Affiliations") {
            if let Some(tag) = entry.get("Tag").and_then(Value::as_str) {
                inv.affiliations.push(Affiliation {
                    tag: tag.to_string(),
                    standing: entry.get("Standing").and_then(Value::as_i64).unwrap_or(0),
                    title: entry.get("Title").and_then(Value::as_i64).unwrap_or(0),
                });
            }
        }
        Ok(inv)
    }

    pub fn parse_text(text: &str) -> crate::Result<Self> {
        Self::parse(serde_json::from_str(text)?)
    }

    /// Total owned count of `unique` across ranks.
    pub fn count_of(&self, unique: &str) -> u64 {
        self.items.iter().filter(|i| i.unique_name == unique).map(|i| i.count).sum::<u64>()
            + self.equipment.iter().filter(|e| e.unique_name == unique).count() as u64
    }

    /// unique name → total count, for fast lookups.
    pub fn count_map(&self) -> HashMap<&str, u64> {
        let mut map: HashMap<&str, u64> = HashMap::new();
        for item in &self.items {
            *map.entry(item.unique_name.as_str()).or_default() += item.count;
        }
        for e in &self.equipment {
            *map.entry(e.unique_name.as_str()).or_default() += 1;
        }
        map
    }
}

/// Inventory category tabs (original: `InventoryGroup`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InventoryGroup {
    Everything,
    Parts,
    Sets,
    Relics,
    Mods,
    Arcanes,
    Equipment,
    Resources,
    Misc,
}

impl InventoryGroup {
    pub const TABS: [InventoryGroup; 9] = [
        InventoryGroup::Everything,
        InventoryGroup::Parts,
        InventoryGroup::Sets,
        InventoryGroup::Relics,
        InventoryGroup::Mods,
        InventoryGroup::Arcanes,
        InventoryGroup::Equipment,
        InventoryGroup::Resources,
        InventoryGroup::Misc,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InventoryGroup::Everything => "Everything",
            InventoryGroup::Parts => "All Parts",
            InventoryGroup::Sets => "Full Sets",
            InventoryGroup::Relics => "Relics",
            InventoryGroup::Mods => "Mods",
            InventoryGroup::Arcanes => "Arcanes",
            InventoryGroup::Equipment => "Equipment",
            InventoryGroup::Resources => "Resources",
            InventoryGroup::Misc => "Misc",
        }
    }
}

/// One display row of the inventory view.
#[derive(Debug, Clone)]
pub struct InventoryRow {
    pub unique_name: String,
    pub name: String,
    pub group: InventoryGroup,
    pub kind: ItemKind,
    pub count: u64,
    pub rank: Option<u32>,
    pub max_rank: Option<u32>,
    pub ducats: Option<u32>,
    pub wfm_slug: Option<String>,
    pub image_url: Option<String>,
    pub is_prime_part: bool,
    pub vaulted: bool,
    /// Equipment only: current level from XP.
    pub level: Option<u32>,
    /// Equipment only: filled archon shard sockets.
    pub shards: Vec<ArchonShard>,
    /// Relics only: warframe.market refinement ("intact" … "radiant").
    pub subtype: Option<&'static str>,
}

impl InventoryRow {
    /// Market pool this row is priced and traded in: relic refinement, or
    /// rank 0 / max rank for ranked items (others share rank 0).
    pub fn variant(&self) -> Variant {
        if let Some(subtype) = self.subtype {
            return Variant::Subtype(subtype);
        }
        match (self.rank, self.max_rank) {
            (Some(r), Some(max)) if max > 0 => Variant::Rank(if r >= max { max } else { 0 }),
            (_, Some(max)) if max > 0 => Variant::Rank(0),
            _ => Variant::Base,
        }
    }
}

/// DE keeps each relic refinement as its own item (`…A1Bronze` … `…A1Platinum`);
/// warframe.market has one item (`…A1`) with refinement subtypes.
pub fn relic_refinement(unique: &str) -> Option<(&str, &'static str)> {
    if !unique.contains("/Projections/") {
        return None;
    }
    [("Bronze", "intact"), ("Silver", "exceptional"), ("Gold", "flawless"), ("Platinum", "radiant")]
        .into_iter()
        .find_map(|(suffix, subtype)| Some((unique.strip_suffix(suffix)?, subtype)))
}

/// Every intrinsic skill tops out at rank 10.
pub const MAX_INTRINSIC_RANK: u64 = 10;

/// Equipment level from XP (frames 1000·r², weapons 500·r²).
pub fn level_from_xp(xp: u64, frame_like: bool, max_level: u32) -> u32 {
    let per = if frame_like { 1000.0 } else { 500.0 };
    ((xp as f64 / per).sqrt().floor() as u32).min(max_level.max(30))
}

/// Groups the snapshot into display rows.
pub fn classify(inv: &Inventory, db: &ItemDb, catalog: &Catalog) -> Vec<InventoryRow> {
    let mut rows = Vec::with_capacity(inv.items.len() + inv.equipment.len());
    for item in &inv.items {
        let info = db.get(&item.unique_name);
        let relic = relic_refinement(&item.unique_name);
        let wfm = catalog.by_game_ref(relic.map_or(item.unique_name.as_str(), |(base, _)| base));
        // Stacks DE's export does not list are internal bookkeeping (Helminth
        // ability "blueprints", stickers, test keys, fake quest items): no name,
        // no art, nothing to show. Equipment below is always real.
        if info.is_none() && wfm.is_none() && !db.relics.contains_key(&item.unique_name) {
            continue;
        }
        // A few real items are missing from DE's export but listed on
        // warframe.market (Maglev, Clashing Forest, Abyssal Beacon): take the
        // name, kind and art from there.
        let market_only = if info.is_none() { wfm } else { None };
        let kind = match (info, market_only) {
            (Some(i), _) => i.kind,
            (None, Some(w)) if w.tags.iter().any(|t| t == "mod") => ItemKind::Mod,
            (None, Some(w)) if w.tags.iter().any(|t| t == "arcane_enhancement") => ItemKind::Arcane,
            _ => ItemKind::Other,
        };
        let name = market_only.map(|w| w.name.clone()).unwrap_or_else(|| db.name(&item.unique_name));
        let tradable = wfm.is_some();
        let group = if db.relics.contains_key(&item.unique_name) {
            InventoryGroup::Relics
        } else {
            match kind {
                ItemKind::Mod => InventoryGroup::Mods,
                ItemKind::Arcane => InventoryGroup::Arcanes,
                ItemKind::Blueprint | ItemKind::Component if tradable => InventoryGroup::Parts,
                ItemKind::Resource | ItemKind::Gear | ItemKind::Endo | ItemKind::Drone => {
                    if tradable { InventoryGroup::Misc } else { InventoryGroup::Resources }
                }
                _ => InventoryGroup::Misc,
            }
        };
        let ducats = info.and_then(|i| i.ducats).or_else(|| wfm.and_then(|w| w.ducats));
        rows.push(InventoryRow {
            is_prime_part: group == InventoryGroup::Parts && name.contains(" Prime ") && ducats.is_some(),
            max_rank: info.and_then(|i| i.max_rank).or_else(|| wfm.and_then(|w| w.max_rank)),
            image_url: db.image_url(&item.unique_name).or_else(|| market_only.and_then(|w| w.thumb_url())),
            wfm_slug: wfm.map(|w| w.slug.clone()),
            vaulted: info.is_some_and(|i| i.vaulted),
            unique_name: item.unique_name.clone(),
            count: item.count,
            rank: item.rank,
            level: None,
            shards: Vec::new(),
            subtype: relic.map(|(_, s)| s),
            name,
            group,
            kind,
            ducats,
        });
    }
    for e in &inv.equipment {
        let info = db.get(&e.unique_name);
        let kind = info.map(|i| i.kind).unwrap_or(ItemKind::Other);
        let max_level = info.map(|i| i.max_level).unwrap_or(30);
        rows.push(InventoryRow {
            unique_name: e.unique_name.clone(),
            name: db.name(&e.unique_name),
            group: InventoryGroup::Equipment,
            kind,
            count: 1,
            rank: None,
            max_rank: None,
            ducats: None,
            // Built equipment is account-bound, never priced.
            wfm_slug: None,
            image_url: db.image_url(&e.unique_name),
            is_prime_part: false,
            vaulted: false,
            level: Some(level_from_xp(e.xp, kind.is_frame_like(), max_level)),
            shards: e.shard_slots.iter().flatten().cloned().collect(),
            subtype: None,
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "PremiumCredits": 120, "RegularCredits": 5000000, "FusionPoints": 30000,
        "PlayerLevel": 22,
        "MiscItems": [{"ItemType": "/Lotus/Types/Items/MiscItems/OrokinCell", "ItemCount": 40},
                      {"ItemType": "/Lotus/Types/Items/MiscItems/PrimeBucks", "ItemCount": 23},
                      {"ItemType": "/Lotus/Types/Game/Projections/T4VoidProjectionXakuPrimeDBronze", "ItemCount": 3}],
        "Recipes": [{"ItemType": "/Lotus/Types/Recipes/WarframeRecipes/AshPrimeChassisBlueprint", "ItemCount": 2}],
        "RawUpgrades": [{"ItemType": "/Lotus/Upgrades/Mods/Rifle/WeaponDamageAmountMod", "ItemCount": 2}],
        "Upgrades": [{"ItemType": "/Lotus/Upgrades/Mods/Rifle/WeaponDamageAmountMod", "UpgradeFingerprint": "{\"lvl\":10}", "ItemId": {"$oid": "a"}},
                     {"ItemType": "/Lotus/Upgrades/Mods/Randomized/LotusRifleRandomModRare",
                      "UpgradeFingerprint": "{\"compat\":\"/Lotus/Weapons/Tenno/Rifle/Soma\",\"lim\":0,\"lvlReq\":12,\"lvl\":8,\"rerolls\":3,\"pol\":\"AP_ATTACK\",\"buffs\":[{\"Tag\":\"WeaponCritChanceMod\",\"Value\":300000000}],\"curses\":[]}",
                      "ItemId": {"$oid": "b"}}],
        "Suits": [{"ItemType": "/Lotus/Powersuits/Ninja/Ninja", "XP": 900000, "ItemId": {"$oid": "c"},
                   "ArchonCrystalUpgrades": [{"UpgradeType": "/Lotus/Upgrades/Invigorations/ArchonCrystalUpgrades/ArchonCrystalUpgradeWarframeHealthMythic", "Color": "ACC_RED_MYTHIC"},
                                             {}, {"UpgradeType": "/Lotus/Upgrades/Invigorations/ArchonCrystalUpgrades/ArchonCrystalUpgradeWarframeArmor", "Color": "ACC_YELLOW"}]}],
        "XPInfo": [{"ItemType": "/Lotus/Powersuits/Ninja/Ninja", "XP": 900000}],
        "PendingRecipes": [{"ItemType": "/Lotus/Types/Recipes/X", "CompletionDate": {"$date": {"$numberLong": "1700000000000"}}}],
        "Affiliations": [{"Tag": "ArbitersSyndicate", "Standing": 1234, "Title": 2}]
    }"#;

    #[test]
    fn relic_refinements_map_to_market_subtypes() {
        let r = relic_refinement;
        assert_eq!(r("/Lotus/Types/Game/Projections/T1VoidProjectionHydroidPrimeBBronze"), Some(("/Lotus/Types/Game/Projections/T1VoidProjectionHydroidPrimeB", "intact")));
        assert_eq!(r("/Lotus/Types/Game/Projections/T1VoidProjectionUPlatinum").map(|x| x.1), Some("radiant"));
        assert_eq!(r("/Lotus/Types/Items/MiscItems/OrokinCellGold"), None);
    }

    #[test]
    fn hides_items_unknown_to_game_data() {
        let inv = Inventory::parse_text(SAMPLE).unwrap();
        let rows = classify(&inv, &ItemDb::default(), &Catalog::default());
        assert!(rows.iter().all(|r| r.group == InventoryGroup::Equipment), "unknown stacks are dropped");
        assert_eq!(rows.len(), inv.equipment.len());
    }

    #[test]
    fn parses_sample() {
        let inv = Inventory::parse_text(SAMPLE).unwrap();
        assert_eq!(inv.currencies.platinum, 120);
        assert_eq!(inv.currencies.ducats, 23);
        assert_eq!(inv.mastery_rank, 22);
        assert_eq!(inv.count_of("/Lotus/Types/Items/MiscItems/OrokinCell"), 40);
        let serration: Vec<_> = inv
            .items
            .iter()
            .filter(|i| i.unique_name.ends_with("WeaponDamageAmountMod"))
            .collect();
        assert_eq!(serration.len(), 2, "rank 0 and rank 10 stay separate");
        assert_eq!(inv.rivens.len(), 1);
        assert_eq!(inv.rivens[0].rerolls, 3);
        assert_eq!(inv.equipment.len(), 1);
        let slots = &inv.equipment[0].shard_slots;
        assert!(slots[1].is_none(), "empty sockets keep their position");
        let shards: Vec<_> = slots.iter().flatten().collect();
        assert_eq!(shards.len(), 2);
        assert_eq!((shards[0].color, shards[0].tauforged), (Some(ShardColor::Crimson), true));
        assert_eq!((shards[1].color, shards[1].tauforged), (Some(ShardColor::Amber), false));
        assert_eq!(shards[0].stat_label(), "Health");
        assert_eq!(shards[1].stat_label(), "Armor");
        assert!(inv.pending_recipes[0].completes_at.is_some());
    }

    #[test]
    fn unwraps_envelope() {
        let wrapped = serde_json::json!({ "InventoryJson": SAMPLE });
        assert!(Inventory::parse(wrapped).is_ok());
        assert!(Inventory::parse(serde_json::json!({"foo": 1})).is_err());
    }

    #[test]
    fn shard_stat_labels() {
        let label = |u: &str| ArchonShard { color: None, tauforged: false, upgrade: u.into() }.stat_label();
        let base = "/Lotus/Upgrades/Invigorations/ArchonCrystalUpgrades/ArchonCrystalUpgrade";
        assert_eq!(label(&format!("{base}WarframeAbilityStrengthMythic")), "Ability Strength");
        assert_eq!(label(&format!("{base}WarframeHPRegen")), "HP Regen");
        assert_eq!(label(&format!("{base}WarframeCastingSpeed")), "Casting Speed");
        assert_eq!(label(""), "");
    }

    #[test]
    fn levels() {
        assert_eq!(level_from_xp(900_000, true, 30), 30);
        assert_eq!(level_from_xp(450_000, false, 30), 30);
        assert_eq!(level_from_xp(800_000, false, 40), 40);
        assert_eq!(level_from_xp(0, false, 30), 0);
    }
}
