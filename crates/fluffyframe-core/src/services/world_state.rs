//! World state (original: `services/worldStateFetch.ts`, `worldStateParser.ts`,
//! `worldStateSteelPath.ts`, `src/lib/world/*`).
//!
//! Reads DE's raw `worldState.php` and resolves node/item names through the item
//! database, so no third-party parser is involved.

use anyhow::Context;
use chrono::{DateTime, Duration, TimeZone, Utc};
use serde_json::Value;

use super::http;
use super::inventory::de_date;
use super::item_database::{ItemDb, fallback_name, store_item_path};
use crate::config::constants::WORLD_STATE_URL;
use crate::config::game_tables::{self, split_camel, title_case};

#[derive(Debug, Clone)]
pub struct Fissure {
    pub tier: String,
    pub mission_type: String,
    pub node: String,
    pub node_id: String,
    pub steel_path: bool,
    pub storm: bool,
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct BaroItem {
    pub unique_name: String,
    pub name: String,
    pub ducats: Option<u32>,
    pub credits: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct VoidTrader {
    pub location: String,
    pub activation: Option<DateTime<Utc>>,
    pub expiry: Option<DateTime<Utc>>,
    pub inventory: Vec<BaroItem>,
}

impl VoidTrader {
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        self.activation.is_some_and(|a| a <= now) && self.expiry.is_some_and(|e| e > now)
    }
}

#[derive(Debug, Clone)]
pub struct SortieMission {
    pub node: String,
    pub mission_type: String,
    pub modifier: String,
}

#[derive(Debug, Clone)]
pub struct Sortie {
    pub boss: String,
    pub expiry: Option<DateTime<Utc>>,
    pub missions: Vec<SortieMission>,
}

#[derive(Debug, Clone)]
pub struct NightwaveChallenge {
    pub title: String,
    pub kind: &'static str,
    pub standing: u32,
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Nightwave {
    pub season: i64,
    pub expiry: Option<DateTime<Utc>>,
    pub challenges: Vec<NightwaveChallenge>,
}

#[derive(Debug, Clone)]
pub struct Invasion {
    pub node: String,
    pub attacker: String,
    pub defender: String,
    pub attacker_reward: String,
    pub defender_reward: String,
    /// 0..1 attacker progress.
    pub progress: f64,
}

#[derive(Debug, Clone)]
pub struct Bounty {
    pub syndicate: String,
    pub expiry: Option<DateTime<Utc>>,
    pub jobs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub node: String,
    pub mission_type: String,
    pub reward: String,
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct Circuit {
    pub normal: Vec<String>,
    pub steel_path: Vec<String>,
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct DarvoDeal {
    pub unique_name: String,
    pub name: String,
    pub original_price: u32,
    pub sale_price: u32,
    pub discount: u32,
    pub total: u32,
    pub sold: u32,
    pub expiry: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ResurgenceItem {
    pub unique_name: String,
    pub name: String,
    pub aya: u32,
}

#[derive(Debug, Clone)]
pub struct PrimeResurgence {
    pub expiry: Option<DateTime<Utc>>,
    /// Warframes and weapons on offer (bundles and cosmetics left out).
    pub featured: Vec<ResurgenceItem>,
    pub total_offers: usize,
}

#[derive(Debug, Clone)]
pub struct SteelPathHonors {
    pub reward: &'static str,
    pub cost: u32,
    pub expiry: DateTime<Utc>,
}

/// Teshin's weekly rotating offer (original: `services/worldStateSteelPath.ts`).
const STEEL_PATH_ROTATION: [(&str, u32); 8] = [
    ("Umbra Forma Blueprint", 150),
    ("50,000 Kuva", 55),
    ("Kitgun Riven Mod", 75),
    ("3x Forma", 75),
    ("Zaw Riven Mod", 75),
    ("30,000 Endo", 150),
    ("Rifle Riven Mod", 75),
    ("Shotgun Riven Mod", 75),
];

pub fn steel_path_honors(now: DateTime<Utc>) -> SteelPathHonors {
    let epoch = Utc.with_ymd_and_hms(2020, 11, 16, 0, 0, 0).unwrap();
    let secs = (now - epoch).num_seconds().max(0);
    let idx = (secs % (8 * 7 * 86_400)) / (7 * 86_400);
    let (reward, cost) = STEEL_PATH_ROTATION[idx as usize % 8];
    SteelPathHonors { reward, cost, expiry: next_weekly_reset(now) }
}

/// Monday 00:00 UTC after `now`.
pub fn next_weekly_reset(now: DateTime<Utc>) -> DateTime<Utc> {
    use chrono::Datelike;
    let midnight = Utc.from_utc_datetime(&now.date_naive().and_hms_opt(0, 0, 0).unwrap());
    let wd = now.weekday().num_days_from_monday() as i64;
    midnight + Duration::days(7 - wd)
}

/// Next 00:00 UTC.
pub fn next_daily_reset(now: DateTime<Utc>) -> DateTime<Utc> {
    Utc.from_utc_datetime(&now.date_naive().and_hms_opt(0, 0, 0).unwrap()) + Duration::days(1)
}

/// Archon shard colour dropped by each archon.
pub fn archon_shard(boss: &str) -> &'static str {
    match boss {
        "Boreal" => "Azure",
        "Amar" => "Crimson",
        "Nira" => "Amber",
        _ => "",
    }
}

#[derive(Debug, Clone)]
pub struct Cycle {
    pub name: &'static str,
    pub state: &'static str,
    pub ends_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct WorldState {
    pub fetched_at: Option<DateTime<Utc>>,
    pub fissures: Vec<Fissure>,
    pub void_trader: Option<VoidTrader>,
    pub sortie: Option<Sortie>,
    pub archon_hunt: Option<Sortie>,
    pub nightwave: Option<Nightwave>,
    pub invasions: Vec<Invasion>,
    pub bounties: Vec<Bounty>,
    pub alerts: Vec<Alert>,
    pub cycles: Vec<Cycle>,
    pub daily_reset: Option<DateTime<Utc>>,
    pub weekly_reset: Option<DateTime<Utc>>,
    pub circuit: Option<Circuit>,
    pub darvo: Option<DarvoDeal>,
    pub prime_resurgence: Option<PrimeResurgence>,
    pub steel_path: Option<SteelPathHonors>,
}

pub fn fetch_raw() -> crate::Result<Value> {
    let text = http::get_text(WORLD_STATE_URL)?;
    serde_json::from_str(&http::strip_control_chars(&text))
        .with_context(|| format!("parse world state from {WORLD_STATE_URL}"))
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}
fn date(v: &Value, key: &str) -> Option<DateTime<Utc>> {
    v.get(key).and_then(de_date)
}

fn active(list: &[Value], now: DateTime<Utc>) -> Option<&Value> {
    list.iter()
        .find(|e| date(e, "Activation").is_none_or(|a| a <= now) && date(e, "Expiry").is_some_and(|x| x > now))
        .or_else(|| list.first())
}

fn enum_tail(value: &str, prefix: &str) -> String {
    title_case(&value.strip_prefix(prefix).unwrap_or(value).replace('_', " "))
}

/// `.../SeasonDailyAimGlide` → "Aim Glide".
fn prettify_challenge(path: &str) -> (String, &'static str, u32) {
    let tail = path.rsplit('/').next().unwrap_or(path);
    let (rest, kind, standing) = if let Some(r) = tail.strip_prefix("SeasonWeeklyHard") {
        (r, "Elite Weekly", 7000)
    } else if let Some(r) = tail.strip_prefix("SeasonWeeklyPermanent") {
        (r, "Weekly", 4500)
    } else if let Some(r) = tail.strip_prefix("SeasonWeekly") {
        (r, "Weekly", 4500)
    } else if let Some(r) = tail.strip_prefix("SeasonDaily") {
        (r, "Daily", 1000)
    } else {
        (tail, "Challenge", 0)
    };
    (split_camel(rest), kind, standing)
}

fn reward_label(reward: &Value, db: &ItemDb) -> String {
    let mut parts = Vec::new();
    for c in arr(reward, "countedItems") {
        let name = db.name(s(c, "ItemType"));
        let n = c.get("ItemCount").and_then(Value::as_u64).unwrap_or(1);
        parts.push(if n > 1 { format!("{n}× {name}") } else { name });
    }
    for item in arr(reward, "items") {
        if let Some(path) = item.as_str() {
            parts.push(db.name(&store_item_path(path)));
        }
    }
    if let Some(cr) = reward.get("credits").and_then(Value::as_u64).filter(|c| *c > 0) {
        parts.push(format!("{cr} cr"));
    }
    parts.join(", ")
}

/// Open-world day/night cycles (original: `src/lib/world/*` cycle math).
pub fn cycles(now: DateTime<Utc>, cetus_expiry: Option<DateTime<Utc>>) -> Vec<Cycle> {
    let mut out = Vec::new();
    // Earth: 4h day, 4h night.
    let secs = now.timestamp();
    let into = secs.rem_euclid(28_800);
    let (state, left) = if into < 14_400 { ("Day", 14_400 - into) } else { ("Night", 28_800 - into) };
    out.push(Cycle { name: "Earth", state, ends_at: now + Duration::seconds(left) });

    // Cetus: 150 min cycle ending at the Ostron bounty expiry, last 50 min are night.
    if let Some(mut end) = cetus_expiry {
        while end <= now {
            end += Duration::minutes(150);
        }
        let night_start = end - Duration::minutes(50);
        let (state, ends_at) = if now < night_start { ("Day", night_start) } else { ("Night", end) };
        out.push(Cycle { name: "Cetus", state, ends_at });
        // The Cambion Drift follows Cetus: day is Fass, night is Vome.
        out.push(Cycle {
            name: "Cambion Drift",
            state: if state == "Day" { "Fass" } else { "Vome" },
            ends_at,
        });
    }

    // Orb Vallis: 1600 s loop, 400 s warm.
    let ms = now.timestamp_millis();
    let since = (ms - 1_541_837_628_000).rem_euclid(1_600_000);
    let (state, left_ms) = if since < 400_000 { ("Warm", 400_000 - since) } else { ("Cold", 1_600_000 - since) };
    out.push(Cycle { name: "Orb Vallis", state, ends_at: now + Duration::milliseconds(left_ms) });
    out
}

pub fn parse(raw: &Value, db: &ItemDb) -> WorldState {
    let now = Utc::now();
    let mut ws = WorldState { fetched_at: Some(now), ..Default::default() };

    for m in arr(raw, "ActiveMissions") {
        let modifier = s(m, "Modifier");
        let Some(tier) = game_tables::void_tier(modifier) else { continue };
        let expiry = date(m, "Expiry");
        if expiry.is_some_and(|e| e < now) {
            continue;
        }
        let node = s(m, "Node");
        ws.fissures.push(Fissure {
            tier: tier.into(),
            mission_type: game_tables::mission_type_label(s(m, "MissionType")),
            node: db.node_label(node),
            node_id: node.into(),
            steel_path: m.get("Hard").and_then(Value::as_bool).unwrap_or(false) || modifier.ends_with("Hard"),
            storm: false,
            expiry,
        });
    }
    for m in arr(raw, "VoidStorms") {
        let raw_tier = s(m, "ActiveMissionTier");
        let Some(tier) = game_tables::void_tier(raw_tier) else { continue };
        let expiry = date(m, "Expiry");
        if expiry.is_some_and(|e| e < now) {
            continue;
        }
        let node = s(m, "Node");
        ws.fissures.push(Fissure {
            tier: tier.into(),
            mission_type: db
                .regions
                .get(node)
                .map(|r| r.mission_type.clone())
                .filter(|m| !m.is_empty())
                .unwrap_or_else(|| "Railjack".into()),
            node: db.node_label(node),
            node_id: node.into(),
            steel_path: raw_tier.ends_with("Hard"),
            storm: true,
            expiry,
        });
    }
    ws.fissures.sort_by(|a, b| {
        (game_tables::void_tier_order(&a.tier), a.expiry).cmp(&(game_tables::void_tier_order(&b.tier), b.expiry))
    });

    if let Some(baro) = arr(raw, "VoidTraders").first() {
        ws.void_trader = Some(VoidTrader {
            location: game_tables::hub_label(s(baro, "Node")),
            activation: date(baro, "Activation"),
            expiry: date(baro, "Expiry"),
            inventory: arr(baro, "Manifest")
                .iter()
                .filter(|i| !s(i, "ItemType").contains("BaroTreasureBox"))
                .map(|i| {
                    let unique = store_item_path(s(i, "ItemType"));
                    BaroItem {
                        name: db.name(&unique),
                        ducats: i.get("PrimePrice").and_then(Value::as_u64).map(|n| n as u32),
                        credits: i.get("RegularPrice").and_then(Value::as_u64),
                        unique_name: unique,
                    }
                })
                .collect(),
        });
    }

    let sortie_from = |v: &Value, missions_key: &str| Sortie {
        boss: enum_tail(s(v, "Boss"), "SORTIE_BOSS_"),
        expiry: date(v, "Expiry"),
        missions: arr(v, missions_key)
            .iter()
            .map(|m| SortieMission {
                node: db.node_label(s(m, "node")),
                mission_type: game_tables::mission_type_label(s(m, "missionType")),
                modifier: {
                    let modifier = s(m, "modifierType");
                    if modifier.is_empty() { String::new() } else { enum_tail(modifier, "SORTIE_MODIFIER_") }
                },
            })
            .collect(),
    };
    ws.sortie = active(arr(raw, "Sorties"), now).map(|v| sortie_from(v, "Variants"));
    ws.archon_hunt = active(arr(raw, "LiteSorties"), now).map(|v| sortie_from(v, "Missions"));

    if let Some(season) = raw.get("SeasonInfo") {
        ws.nightwave = Some(Nightwave {
            season: season.get("Season").and_then(Value::as_i64).unwrap_or(0),
            expiry: date(season, "Expiry"),
            challenges: arr(season, "ActiveChallenges")
                .iter()
                .filter(|c| date(c, "Expiry").is_none_or(|e| e > now))
                .map(|c| {
                    let (title, kind, standing) = prettify_challenge(s(c, "Challenge"));
                    NightwaveChallenge { title, kind, standing, expiry: date(c, "Expiry") }
                })
                .collect(),
        });
    }

    for inv in arr(raw, "Invasions") {
        if inv.get("Completed").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        let goal = inv.get("Goal").and_then(Value::as_f64).unwrap_or(1.0).max(1.0);
        let count = inv.get("Count").and_then(Value::as_f64).unwrap_or(0.0);
        ws.invasions.push(Invasion {
            node: db.node_label(s(inv, "Node")),
            attacker: game_tables::faction_label(s(inv, "Faction")),
            defender: game_tables::faction_label(s(inv, "DefenderFaction")),
            attacker_reward: inv.get("AttackerReward").map(|r| reward_label(r, db)).unwrap_or_default(),
            defender_reward: inv.get("DefenderReward").map(|r| reward_label(r, db)).unwrap_or_default(),
            progress: ((count / goal + 1.0) / 2.0).clamp(0.0, 1.0),
        });
    }

    let mut cetus_expiry = None;
    for m in arr(raw, "SyndicateMissions") {
        let tag = s(m, "Tag");
        if tag == "CetusSyndicate" {
            cetus_expiry = date(m, "Expiry");
        }
        let jobs = arr(m, "Jobs");
        if jobs.is_empty() {
            continue;
        }
        ws.bounties.push(Bounty {
            syndicate: game_tables::syndicate_label(tag),
            expiry: date(m, "Expiry"),
            jobs: jobs
                .iter()
                .map(|j| {
                    let job = fallback_name(s(j, "jobType"));
                    let lo = j.get("minEnemyLevel").and_then(Value::as_u64).unwrap_or(0);
                    let hi = j.get("maxEnemyLevel").and_then(Value::as_u64).unwrap_or(0);
                    format!("{job} (Lv {lo}-{hi})")
                })
                .collect(),
        });
    }

    for a in arr(raw, "Alerts") {
        let expiry = date(a, "Expiry");
        if expiry.is_some_and(|e| e < now) {
            continue;
        }
        let info = a.get("MissionInfo").unwrap_or(&Value::Null);
        ws.alerts.push(Alert {
            node: db.node_label(s(info, "location")),
            mission_type: game_tables::mission_type_label(s(info, "missionType")),
            reward: info.get("missionReward").map(|r| reward_label(r, db)).unwrap_or_default(),
            expiry,
        });
    }

    ws.cycles = cycles(now, cetus_expiry);
    ws.daily_reset = Some(next_daily_reset(now));
    ws.weekly_reset = Some(next_weekly_reset(now));
    ws.steel_path = Some(steel_path_honors(now));

    if let Some(c) = active(arr(raw, "EndlessXpSchedule"), now) {
        let pick = |cat: &str| -> Vec<String> {
            arr(c, "CategoryChoices")
                .iter()
                .find(|x| s(x, "Category") == cat)
                .map(|x| arr(x, "Choices").iter().filter_map(Value::as_str).map(split_camel).collect())
                .unwrap_or_default()
        };
        ws.circuit = Some(Circuit { normal: pick("EXC_NORMAL"), steel_path: pick("EXC_HARD"), expiry: date(c, "Expiry") });
    }

    if let Some(d) = arr(raw, "DailyDeals").first() {
        let unique = store_item_path(s(d, "StoreItem"));
        let n = |k: &str| d.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
        ws.darvo = Some(DarvoDeal {
            name: db.name(&unique),
            original_price: n("OriginalPrice"),
            sale_price: n("SalePrice"),
            discount: n("Discount"),
            total: n("AmountTotal"),
            sold: n("AmountSold"),
            expiry: date(d, "Expiry"),
            unique_name: unique,
        });
    }

    if let Some(v) = arr(raw, "PrimeVaultTraders").first() {
        let manifest = arr(v, "Manifest");
        let featured = manifest
            .iter()
            .map(|i| (store_item_path(s(i, "ItemType")), i.get("PrimePrice").and_then(Value::as_u64).unwrap_or(0) as u32))
            .filter(|(u, _)| db.get(u).is_some_and(|info| info.kind.is_equipment()))
            .map(|(u, aya)| ResurgenceItem { name: db.name(&u), unique_name: u, aya })
            .collect();
        ws.prime_resurgence = Some(PrimeResurgence { expiry: date(v, "Expiry"), featured, total_offers: manifest.len() });
    }
        ws
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_are_in_future() {
        let now = Utc::now();
        for c in cycles(now, Some(now + Duration::minutes(30))) {
            assert!(c.ends_at > now, "{} ends in the past", c.name);
        }
    }

    #[test]
    fn cetus_night_is_last_50_minutes() {
        let now = Utc::now();
        let c = cycles(now, Some(now + Duration::minutes(30)));
        let cetus = c.iter().find(|c| c.name == "Cetus").unwrap();
        assert_eq!(cetus.state, "Night");
    }

    #[test]
    fn parses_minimal_world_state() {
        let now_ms = Utc::now().timestamp_millis();
        let raw = serde_json::json!({
            "ActiveMissions": [{ "Node": "SolNode1", "MissionType": "MT_SURVIVAL", "Modifier": "VoidT2",
                "Expiry": {"$date": {"$numberLong": (now_ms + 600_000).to_string()}} }],
            "VoidTraders": [{ "Node": "EarthHUB", "Activation": {"$date": {"$numberLong": "1"}},
                "Expiry": {"$date": {"$numberLong": (now_ms + 1000).to_string()}}, "Manifest": [] }]
        });
        let ws = parse(&raw, &ItemDb::default());
        assert_eq!(ws.fissures.len(), 1);
        assert_eq!(ws.fissures[0].tier, "Meso");
        assert!(ws.void_trader.unwrap().is_active(Utc::now()));
    }

    #[test]
    fn steel_path_rotation() {
        // 2020-11-16 starts the rotation at index 0; one week later is index 1.
        let start = Utc.with_ymd_and_hms(2020, 11, 16, 12, 0, 0).unwrap();
        assert_eq!(steel_path_honors(start).reward, "Umbra Forma Blueprint");
        assert_eq!(steel_path_honors(start + Duration::days(7)).reward, "50,000 Kuva");
        assert_eq!(steel_path_honors(start + Duration::days(56)).reward, "Umbra Forma Blueprint");
        use chrono::Datelike;
        assert_eq!(next_weekly_reset(start).weekday(), chrono::Weekday::Mon);
    }

    #[test]
    fn challenge_names() {
        let (t, k, st) = prettify_challenge("/Lotus/Types/Challenges/Seasons/Weekly/SeasonWeeklyHardKillEnemies");
        assert_eq!((t.as_str(), k, st), ("Kill Enemies", "Elite Weekly", 7000));
    }
}
