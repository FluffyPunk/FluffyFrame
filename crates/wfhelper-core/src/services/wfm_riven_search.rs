//! Riven market lookups (original: `services/wfmRivenItems.ts`, `wfmRivenSearch.ts`).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::json_cache;
use super::wfm_client::{self, Api};
use crate::config::constants::WFM_CATALOG_TTL;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RivenWeapon {
    pub slug: String,
    pub name: String,
    pub game_ref: Option<String>,
    pub riven_type: Option<String>,
    pub disposition: Option<f64>,
}

#[derive(Default)]
pub struct RivenWeapons {
    pub weapons: Vec<RivenWeapon>,
    by_game_ref: HashMap<String, usize>,
}

impl RivenWeapons {
    pub fn by_game_ref(&self, unique: &str) -> Option<&RivenWeapon> {
        self.by_game_ref.get(&unique.to_lowercase()).map(|&i| &self.weapons[i])
    }

    pub fn search(&self, q: &str, limit: usize) -> Vec<&RivenWeapon> {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<_> = self.weapons.iter().filter(|w| w.name.to_lowercase().contains(&q)).collect();
        hits.sort_by_key(|w| (!w.name.to_lowercase().starts_with(&q), w.name.len()));
        hits.truncate(limit);
        hits
    }
}

pub fn load_weapons() -> crate::Result<RivenWeapons> {
    let text = json_cache::fetch_cached("wfm/riven-weapons.json", WFM_CATALOG_TTL, || {
        Ok(wfm_client::get(Api::V2, "/riven/weapons")?.to_string())
    })?;
    let data = wfm_client::unwrap_v2(serde_json::from_str(&text)?)?;
    let weapons: Vec<RivenWeapon> = data
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|w| {
                    Some(RivenWeapon {
                        slug: w.get("slug")?.as_str()?.to_string(),
                        name: w.pointer("/i18n/en/name")?.as_str()?.to_string(),
                        game_ref: w.get("gameRef").and_then(Value::as_str).map(str::to_string),
                        riven_type: w.get("rivenType").and_then(Value::as_str).map(str::to_string),
                        disposition: w.get("disposition").and_then(Value::as_f64),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let by_game_ref = weapons
        .iter()
        .enumerate()
        .filter_map(|(i, w)| Some((w.game_ref.as_ref()?.to_lowercase(), i)))
        .collect();
    Ok(RivenWeapons { weapons, by_game_ref })
}

#[derive(Debug, Clone)]
pub struct RivenAuction {
    pub id: String,
    pub buyout: Option<u32>,
    pub starting: Option<u32>,
    pub owner: String,
    pub owner_status: String,
    pub name: String,
    pub rerolls: u32,
    pub mastery: u32,
    pub polarity: String,
    /// (warframe.market `url_name`, value, positive)
    pub attributes: Vec<(String, f64, bool)>,
    /// Fixed-price sale rather than a bidding auction.
    pub direct_sell: bool,
}

/// Stats a riven search can filter on: warframe.market name and label, one per
/// market name (Damage and Melee Damage share one), by label (original:
/// `getRivenStatOptions`).
pub fn stat_options() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for tag in crate::config::game_tables::RIVEN_STAT_TAGS {
        if let Some(name) = wfm_attribute(tag)
            && !out.iter().any(|(n, _)| *n == name)
        {
            out.push((name, crate::config::game_tables::riven_stat_label(tag, false)));
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// warframe.market's riven attribute name for a DE upgrade tag
/// (original: `config/shared/wfmRivenVocabulary.ts`).
pub fn wfm_attribute(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "WeaponCritChanceMod" => "critical_chance",
        "WeaponCritDamageMod" => "critical_damage",
        "WeaponFireIterationsMod" => "multishot",
        "WeaponFireRateMod" => "fire_rate_/_attack_speed",
        "WeaponDamageAmountMod" | "WeaponMeleeDamageMod" => "base_damage_/_melee_damage",
        "WeaponReloadSpeedMod" => "reload_speed",
        "WeaponStunChanceMod" => "status_chance",
        "WeaponProcTimeMod" => "status_duration",
        "WeaponPunctureDepthMod" => "punch_through",
        "WeaponClipMaxMod" => "magazine_capacity",
        "WeaponAmmoMaxMod" => "ammo_maximum",
        "WeaponRecoilReductionMod" => "recoil",
        "WeaponZoomFovMod" => "zoom",
        "WeaponProjectileSpeedMod" => "projectile_speed",
        "WeaponImpactDamageMod" => "impact_damage",
        "WeaponArmorPiercingDamageMod" => "puncture_damage",
        "WeaponSlashDamageMod" => "slash_damage",
        "WeaponFreezeDamageMod" => "cold_damage",
        "WeaponFireDamageMod" => "heat_damage",
        "WeaponElectricityDamageMod" => "electric_damage",
        "WeaponToxinDamageMod" => "toxin_damage",
        "WeaponFactionDamageGrineer" | "WeaponMeleeFactionDamageGrineer" => "damage_vs_grineer",
        "WeaponFactionDamageCorpus" | "WeaponMeleeFactionDamageCorpus" => "damage_vs_corpus",
        "WeaponFactionDamageInfested" | "WeaponMeleeFactionDamageInfested" => "damage_vs_infested",
        "WeaponMeleeRangeIncMod" => "range",
        "ComboDurationMod" => "combo_duration",
        "SlideAttackCritChanceMod" => "critical_chance_on_slide_attack",
        "WeaponMeleeFinisherDamageMod" => "finisher_damage",
        "WeaponMeleeComboEfficiencyMod" => "channeling_efficiency",
        "WeaponMeleeComboInitialBonusMod" => "channeling_damage",
        "WeaponMeleeComboPointsOnHitMod" => "chance_to_gain_combo_count",
        "WeaponMeleeComboBonusOnHitMod" => "chance_to_gain_extra_combo_count",
        "WeaponBlastDamageMod" => "blast",
        "WeaponCorrosiveDamageMod" => "corrosive",
        "WeaponGasDamageMod" => "gas",
        "WeaponMagneticDamageMod" => "magnetic",
        "WeaponRadiationDamageMod" => "radiation",
        "WeaponViralDamageMod" => "viral",
        "WeaponFactionDamageOrokin" => "damage_to_orokin",
        "WeaponFactionDamageScaldra" => "damage_to_scaldra",
        "WeaponFactionDamageTechrot" => "damage_to_techrot",
        "WeaponAmmoEfficiency" => "ammo_efficiency",
        "WeaponStatusDamage" => "status_damage",
        "WeaponWeakpointDamage" => "weak_point_damage",
        "WeaponWeakpointCriticalChance" => "weak_point_critical_chance",
        "WeaponMagazineReloadHolstered" => "magazine_reloaded_s_when_holstered",
        "WeaponMeleeHeavyAttackChargeMod" => "heavy_attack_wind_up_speed",
        "WeaponMeleeHeavyAttackDamageMod" => "melee_damage_on_heavy_attack",
        "WeaponMeleeSlamDamageMod" => "slam_attack_damage",
        "WeaponMeleeParryAngleMod" => "parry_angle",
        _ => return None,
    })
}

impl RivenAuction {
    pub fn url(&self) -> String {
        format!("https://warframe.market/auction/{}", self.id)
    }
    pub fn price(&self) -> Option<u32> {
        self.buyout.or(self.starting)
    }
}

/// Cheapest live auctions for a weapon.
pub fn search_auctions(weapon_slug: &str) -> crate::Result<Vec<RivenAuction>> {
    let path = format!("/auctions/search?type=riven&weapon_url_name={weapon_slug}&sort_by=price_asc");
    let json = wfm_client::get(Api::V1, &path)?;
    let list = json.pointer("/payload/auctions").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|a| {
            let item = a.get("item")?;
            Some(RivenAuction {
                id: a.get("id")?.as_str()?.to_string(),
                buyout: a.get("buyout_price").and_then(Value::as_u64).map(|n| n as u32),
                starting: a.get("starting_price").and_then(Value::as_u64).map(|n| n as u32),
                owner: a.pointer("/owner/ingame_name").and_then(Value::as_str).unwrap_or("?").to_string(),
                owner_status: a.pointer("/owner/status").and_then(Value::as_str).unwrap_or("offline").to_string(),
                name: item.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                rerolls: item.get("re_rolls").and_then(Value::as_u64).unwrap_or(0) as u32,
                mastery: item.get("mastery_level").and_then(Value::as_u64).unwrap_or(0) as u32,
                polarity: item.get("polarity").and_then(Value::as_str).unwrap_or("").to_string(),
                direct_sell: a.get("is_direct_sell").and_then(Value::as_bool).unwrap_or(false),
                attributes: item
                    .get("attributes")
                    .and_then(Value::as_array)
                    .map(|attrs| {
                        attrs
                            .iter()
                            .filter_map(|x| {
                                Some((
                                    x.get("url_name")?.as_str()?.to_string(),
                                    x.get("value").and_then(Value::as_f64).unwrap_or(0.0),
                                    x.get("positive").and_then(Value::as_bool).unwrap_or(true),
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect())
}
