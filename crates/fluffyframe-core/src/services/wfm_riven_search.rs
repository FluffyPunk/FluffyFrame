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
    pub weapon_slug: String,
    pub mod_rank: u32,
    /// Highest bid so far (auctions).
    pub top_bid: Option<u32>,
    pub min_reputation: u32,
    pub note: String,
    /// Shown in search (the owner can hide it).
    pub visible: bool,
    pub private: bool,
    pub closed: bool,
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
    Ok(parse_auctions(&wfm_client::get(Api::V1, &path)?))
}

/// `payload.auctions` of a v1 auction listing.
fn parse_auctions(json: &Value) -> Vec<RivenAuction> {
    let list = json.pointer("/payload/auctions").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]);
    list.iter().filter_map(parse_auction).collect()
}

fn parse_auction(a: &Value) -> Option<RivenAuction> {
    let item = a.get("item")?;
    let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_u64).map(|n| n as u32);
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let flag = |k: &str, default: bool| a.get(k).and_then(Value::as_bool).unwrap_or(default);
    Some(RivenAuction {
        id: a.get("id")?.as_str()?.to_string(),
        buyout: num(a, "buyout_price"),
        starting: num(a, "starting_price"),
        owner: a.pointer("/owner/ingame_name").and_then(Value::as_str).unwrap_or("?").to_string(),
        owner_status: a.pointer("/owner/status").and_then(Value::as_str).unwrap_or("offline").to_string(),
        name: text(item, "name"),
        rerolls: num(item, "re_rolls").unwrap_or(0),
        mastery: num(item, "mastery_level").unwrap_or(0),
        polarity: text(item, "polarity"),
        direct_sell: flag("is_direct_sell", false),
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
        weapon_slug: text(item, "weapon_url_name"),
        mod_rank: num(item, "mod_rank").unwrap_or(0),
        top_bid: num(a, "top_bid"),
        min_reputation: num(a, "minimal_reputation").unwrap_or(0),
        note: text(a, "note"),
        visible: flag("visible", true),
        private: flag("private", false),
        closed: flag("closed", false),
    })
}

/// The signed-in user's open riven listings ("contracts"). `profile` is their
/// warframe.market slug; signed-in requests also return private ones.
pub fn my_auctions(profile: &str) -> crate::Result<Vec<RivenAuction>> {
    let json = wfm_client::request(Api::V1, reqwest::Method::GET, &format!("/profile/{profile}/auctions"), None, true)?;
    Ok(parse_auctions(&json).into_iter().filter(|a| !a.closed).collect())
}

impl RivenAuction {
    /// `PUT /v1/auctions/entry/{id}` body: a new price (a direct sale's price, or
    /// an auction's starting bid) and visibility, keeping the rest as listed.
    pub fn update_body(&self, price: u32, visible: bool) -> Value {
        let (starting, buyout) = if self.direct_sell { (price, Some(price)) } else { (price, self.buyout) };
        serde_json::json!({
            "starting_price": starting,
            // Required: null means no buyout.
            "buyout_price": buyout.filter(|b| *b > 0),
            "minimal_reputation": self.min_reputation,
            "visible": visible,
            "note": self.note,
        })
    }
}

pub fn update_auction(auction: &RivenAuction, price: u32, visible: bool) -> crate::Result<()> {
    let path = format!("/auctions/entry/{}", auction.id);
    wfm_client::request(Api::V1, reqwest::Method::PUT, &path, Some(&auction.update_body(price, visible)), true)?;
    Ok(())
}

/// Removes a listing. warframe.market has no delete for auctions: its own
/// remove button closes them this way.
pub fn close_auction(id: &str) -> crate::Result<()> {
    wfm_client::request(Api::V1, reqwest::Method::PUT, &format!("/auctions/entry/{id}/close"), None, true)?;
    Ok(())
}

/// How a riven goes up for sale on warframe.market.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingKind {
    /// Fixed price: buyout-only (starting price and buyout are the same).
    Direct { price: u32 },
    /// Bidding from `starting`, optionally ending early at `buyout`.
    Auction { starting: u32, buyout: Option<u32>, min_reputation: u32 },
}

/// One of the user's rivens, ready to list.
#[derive(Debug, Clone)]
pub struct RivenListing {
    pub weapon_slug: String,
    /// Generated name only ("crita-visicron"), not the weapon.
    pub name: String,
    /// (stat tag, card value at `mod_rank`, positive).
    pub attributes: Vec<(String, f64, bool)>,
    pub rerolls: u32,
    pub mastery_level: u32,
    /// Game polarity (`AP_ATTACK`…).
    pub polarity: Option<String>,
    pub mod_rank: u32,
    pub kind: ListingKind,
    pub private: bool,
    pub note: String,
}

/// warframe.market's riven polarities; it rejects any other value, so an
/// unknown one becomes madurai (as the original does).
fn wfm_polarity(polarity: Option<&str>) -> &'static str {
    match polarity {
        Some("AP_DEFENSE") => "vazarin",
        Some("AP_TACTIC") => "naramon",
        _ => "madurai",
    }
}

impl RivenListing {
    /// `POST /v1/auctions/create` body (original: `createRivenAuction`). Fails on
    /// a stat warframe.market has no attribute for.
    pub fn body(&self) -> crate::Result<Value> {
        let attributes = self
            .attributes
            .iter()
            .map(|(tag, value, positive)| {
                let url_name = wfm_attribute(tag).ok_or_else(|| anyhow::anyhow!("warframe.market has no attribute for {tag}"))?;
                Ok(serde_json::json!({ "url_name": url_name, "value": value, "positive": positive }))
            })
            .collect::<crate::Result<Vec<Value>>>()?;
        let (starting, buyout, min_reputation) = match self.kind {
            ListingKind::Direct { price } => (price, Some(price), 0),
            ListingKind::Auction { starting, buyout, min_reputation } => (starting, buyout.filter(|b| *b > 0), min_reputation),
        };
        let mut body = serde_json::json!({
            "item": {
                "type": "riven",
                "name": self.name.to_lowercase(),
                "weapon_url_name": self.weapon_slug,
                "attributes": attributes,
                "re_rolls": self.rerolls,
                "mastery_level": self.mastery_level,
                "polarity": wfm_polarity(self.polarity.as_deref()),
                "mod_rank": self.mod_rank,
            },
            "starting_price": starting,
            // Required: null means no buyout.
            "buyout_price": buyout,
            "minimal_reputation": min_reputation,
            "private": self.private,
        });
        if !self.note.trim().is_empty() {
            body["note"] = Value::String(self.note.trim().to_string());
        }
        Ok(body)
    }
}

/// Lists a riven; returns the new auction's id.
pub fn create_auction(listing: &RivenListing) -> crate::Result<String> {
    let body = listing.body()?;
    log::info!("listing riven for {} ({} attributes)", listing.weapon_slug, listing.attributes.len());
    let data = wfm_client::request(Api::V1, reqwest::Method::POST, "/auctions/create", Some(&body), true)?;
    Ok(data.pointer("/payload/auction/id").and_then(Value::as_str).unwrap_or_default().to_string())
}

#[cfg(test)]
mod listing_tests {
    use super::*;

    fn listing(kind: ListingKind) -> RivenListing {
        RivenListing {
            weapon_slug: "soma".into(),
            name: "Crita-visicron".into(),
            attributes: vec![("WeaponCritChanceMod".into(), 164.3, true), ("WeaponRecoilReductionMod".into(), 74.5, false)],
            rerolls: 12,
            mastery_level: 16,
            polarity: Some("AP_TACTIC".into()),
            mod_rank: 8,
            kind,
            private: false,
            note: " ".into(),
        }
    }

    #[test]
    fn direct_sale_is_buyout_only() {
        let b = listing(ListingKind::Direct { price: 250 }).body().unwrap();
        assert_eq!((b["starting_price"].as_u64(), b["buyout_price"].as_u64()), (Some(250), Some(250)));
        assert_eq!(b["item"]["name"], "crita-visicron");
        assert_eq!(b["item"]["polarity"], "naramon");
        assert_eq!(b["item"]["attributes"][0]["url_name"], "critical_chance");
        assert_eq!(b["item"]["attributes"][1]["positive"], false);
        assert!(b.get("note").is_none());
    }

    #[test]
    fn auction_without_buyout_sends_null() {
        let b = listing(ListingKind::Auction { starting: 100, buyout: None, min_reputation: 5 }).body().unwrap();
        assert!(b["buyout_price"].is_null());
        assert_eq!(b["minimal_reputation"], 5);
    }

    #[test]
    fn parses_own_listings() {
        let json = serde_json::json!({ "payload": { "auctions": [
            { "id": "a1", "is_direct_sell": true, "buyout_price": 250, "starting_price": 250, "top_bid": null, "visible": false,
              "private": false, "closed": false, "minimal_reputation": 0, "note": "pm me",
              "item": { "name": "crita-visicron", "weapon_url_name": "soma", "mod_rank": 8, "re_rolls": 12, "mastery_level": 16,
                        "polarity": "naramon", "attributes": [{ "url_name": "critical_chance", "value": 164.3, "positive": true }] } },
            { "id": "a2", "closed": true, "item": { "weapon_url_name": "soma" } },
        ] } });
        let list = parse_auctions(&json);
        assert_eq!(list.len(), 2);
        let a = &list[0];
        assert!(a.direct_sell && !a.visible && a.top_bid.is_none());
        assert_eq!((a.weapon_slug.as_str(), a.mod_rank, a.note.as_str()), ("soma", 8, "pm me"));
        assert!(list[1].closed);
        // A direct sale keeps buyout = price; an auction keeps its buyout.
        let b = a.update_body(300, true);
        assert_eq!((b["starting_price"].as_u64(), b["buyout_price"].as_u64(), b["visible"].as_bool()), (Some(300), Some(300), Some(true)));
        let auction = RivenAuction { direct_sell: false, buyout: None, ..a.clone() };
        assert!(auction.update_body(90, false)["buyout_price"].is_null());
    }

    #[test]
    fn unknown_stat_is_refused() {
        let mut l = listing(ListingKind::Direct { price: 1 });
        l.attributes.push(("NotAStat".into(), 1.0, true));
        assert!(l.body().is_err());
    }
}
