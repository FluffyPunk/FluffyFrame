//! 48-hour closed-volume average prices (original: `services/wfmStatsPrice.ts`,
//! `config/shared/wfmStats.ts`).
//!
//! Price = Σ(wa_price·volume) / Σ(volume) over the last 48h of closed sell
//! statistics for the asked rank, skipping buckets more than 10× the live sell
//! book's median (fake closes).

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::atomic_file;
use super::wfm_client::{self, Api};
use crate::config::{constants::WFM_PRICE_TTL, paths};

const WINDOW_HOURS: i64 = 48;
const OUTLIER_FACTOR: f64 = 10.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Price {
    pub average: f64,
    pub volume: f64,
    /// Unix seconds when this price was fetched.
    pub fetched_at: i64,
}

impl Price {
    pub fn is_fresh(&self) -> bool {
        Utc::now().timestamp() - self.fetched_at < WFM_PRICE_TTL.as_secs() as i64
    }
}

/// Which market pool an item trades in: one price for plain items, one per
/// rank for mods/arcanes, one per refinement for relics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Variant {
    #[default]
    Base,
    Rank(u32),
    /// warframe.market subtype: "intact", "exceptional", "flawless", "radiant".
    Subtype(&'static str),
}

impl Variant {
    pub fn rank(rank: Option<u32>) -> Self {
        rank.map_or(Variant::Base, Variant::Rank)
    }

    /// Query string for v2 order endpoints ("" / "?rank=3" / "?subtype=radiant").
    pub fn query(self) -> String {
        match self {
            Variant::Base => String::new(),
            Variant::Rank(r) => format!("?rank={r}"),
            Variant::Subtype(s) => format!("?subtype={s}"),
        }
    }

    /// Short label for order boxes: "R3", "Radiant", "".
    pub fn label(self) -> String {
        match self {
            Variant::Base => String::new(),
            Variant::Rank(r) => format!("R{r}"),
            Variant::Subtype(s) => crate::config::game_tables::title_case(s),
        }
    }
}

/// warframe.market's relic refinement subtypes as static names for [`Variant`].
pub fn relic_subtype(name: &str) -> Option<&'static str> {
    ["intact", "exceptional", "flawless", "radiant"].into_iter().find(|s| *s == name)
}

/// Cache key: slug plus rank or refinement.
pub fn price_key(slug: &str, variant: Variant) -> String {
    match variant {
        Variant::Base => slug.to_string(),
        Variant::Rank(r) => format!("{slug}#{r}"),
        Variant::Subtype(s) => format!("{slug}@{s}"),
    }
}

fn row_rank(row: &Value) -> Option<Option<u32>> {
    match row.get("mod_rank").or_else(|| row.get("rank")) {
        None | Some(Value::Null) => Some(None),
        Some(v) => v.as_u64().map(|n| Some(n as u32)),
    }
}

/// Whether a stats row belongs to the asked pool.
fn in_pool(row: &Value, variant: Variant) -> bool {
    if let Variant::Subtype(want) = variant {
        return row.get("subtype").and_then(Value::as_str) == Some(want);
    }
    match (row_rank(row), variant) {
        (None, _) => false,
        (Some(r), Variant::Rank(want)) => r == Some(want),
        (Some(r), _) => r.is_none() || r == Some(0),
    }
}

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let m = v.len() / 2;
    Some(if v.len() % 2 == 1 { v[m] } else { (v[m - 1] + v[m]) / 2.0 })
}

fn parse_time(row: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(row.get("datetime")?.as_str()?)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn reference_price(payload: &Value, rank: Variant, window_start: DateTime<Utc>) -> Option<f64> {
    let live: Vec<f64> = payload
        .pointer("/statistics_live/48hours")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|r| r.get("order_type").and_then(Value::as_str) == Some("sell") && in_pool(r, rank))
        .filter_map(|r| r.get("median").and_then(Value::as_f64).filter(|m| *m > 0.0))
        .collect();
    if !live.is_empty() {
        return median(live);
    }
    let history: Vec<f64> = payload
        .pointer("/statistics_closed/90days")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|r| in_pool(r, rank) && parse_time(r).is_some_and(|t| t < window_start))
        .filter_map(|r| r.get("median").and_then(Value::as_f64).filter(|m| *m > 0.0))
        .collect();
    median(history)
}

/// The 48h closed-volume average from a v1 statistics response.
pub fn extract_average(json: &Value, rank: Variant, now: DateTime<Utc>) -> Option<(f64, f64)> {
    let payload = json.get("payload")?;
    let rows = payload.pointer("/statistics_closed/48hours")?.as_array()?;
    let window_start = now - chrono::Duration::hours(WINDOW_HOURS);
    let samples: Vec<(f64, f64)> = rows
        .iter()
        .filter(|r| r.get("order_type").and_then(Value::as_str).is_none_or(|t| t == "sell"))
        .filter(|r| in_pool(r, rank))
        .filter_map(|r| {
            let t = parse_time(r)?;
            let price = r.get("wa_price").and_then(Value::as_f64)?;
            let volume = r.get("volume").and_then(Value::as_f64)?;
            (t >= window_start && t <= now && price > 0.0 && volume > 0.0).then_some((price, volume))
        })
        .collect();
    if samples.is_empty() {
        return None;
    }
    let reference = reference_price(payload, rank, window_start);
    let (mut total, mut volume) = (0.0, 0.0);
    for (price, vol) in samples {
        if reference.is_some_and(|r| r > 0.0 && price > r * OUTLIER_FACTOR) {
            continue;
        }
        total += price * vol;
        volume += vol;
    }
    // Hundredths: bulk items trade below 1p each (Ayatan Cyan Star ~0.83p).
    (volume > 0.0).then(|| (((total / volume) * 100.0).round() / 100.0, volume)).filter(|(a, _)| *a > 0.0)
}

/// Persistent price cache shared by all workers.
pub struct PriceCache {
    map: Mutex<HashMap<String, Price>>,
}

impl PriceCache {
    pub fn load() -> Self {
        let map: HashMap<String, Price> =
            atomic_file::read_json(&paths::cache_file("wfm/prices.json")).unwrap_or_default();
        Self { map: Mutex::new(map) }
    }

    pub fn get(&self, key: &str) -> Option<Price> {
        self.map.lock().unwrap().get(key).copied()
    }

    pub fn snapshot(&self) -> HashMap<String, Price> {
        self.map.lock().unwrap().clone()
    }

    pub fn insert(&self, key: String, price: Price) {
        self.map.lock().unwrap().insert(key, price);
    }

    pub fn save(&self) {
        let map = self.snapshot();
        if let Err(e) = atomic_file::write_json(&paths::cache_file("wfm/prices.json"), &map) {
            log::warn!("price cache save failed: {e}");
        }
    }
}

/// Fetches one price from warframe.market (rate-limited).
pub fn fetch(slug: &str, rank: Variant) -> crate::Result<Option<Price>> {
    let json = wfm_client::get(Api::V1, &format!("/items/{slug}/statistics"))?;
    Ok(extract_average(&json, rank, Utc::now()).map(|(average, volume)| Price {
        average,
        volume,
        fetched_at: Utc::now().timestamp(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_weighted_with_outlier() {
        let now = Utc::now();
        let t = |h: i64| (now - chrono::Duration::hours(h)).to_rfc3339();
        let json = serde_json::json!({ "payload": {
            "statistics_closed": { "48hours": [
                { "datetime": t(1), "volume": 2, "wa_price": 10.0, "median": 10.0 },
                { "datetime": t(2), "volume": 1, "wa_price": 13.0, "median": 13.0 },
                { "datetime": t(3), "volume": 1, "wa_price": 5000.0, "median": 5000.0 },
                { "datetime": t(60), "volume": 9, "wa_price": 99.0, "median": 99.0 }
            ]},
            "statistics_live": { "48hours": [
                { "datetime": t(1), "order_type": "sell", "median": 12.0 }
            ]}
        }});
        let (avg, vol) = extract_average(&json, Variant::Base, now).unwrap();
        assert_eq!(vol, 3.0);
        assert_eq!(avg, 11.0); // (20 + 13) / 3 = 11
    }

    #[test]
    fn relic_refinements_and_sub_platinum_prices() {
        let now = Utc::now();
        let json = serde_json::json!({ "payload": { "statistics_closed": { "48hours": [
            { "datetime": now.to_rfc3339(), "volume": 4, "wa_price": 5.0, "subtype": "intact" },
            { "datetime": now.to_rfc3339(), "volume": 2, "wa_price": 25.0, "subtype": "radiant" },
            { "datetime": now.to_rfc3339(), "volume": 6, "wa_price": 0.83, "subtype": "flawless" }
        ]}}});
        assert_eq!(extract_average(&json, Variant::Subtype("radiant"), now).unwrap().0, 25.0);
        assert_eq!(extract_average(&json, Variant::Subtype("intact"), now).unwrap().0, 5.0);
        assert_eq!(extract_average(&json, Variant::Subtype("flawless"), now).unwrap().0, 0.83, "kept below 1p");
        assert_eq!(price_key("lith_a1_relic", Variant::Subtype("radiant")), "lith_a1_relic@radiant");
    }

    #[test]
    fn rank_pools() {
        let now = Utc::now();
        let json = serde_json::json!({ "payload": { "statistics_closed": { "48hours": [
            { "datetime": now.to_rfc3339(), "volume": 1, "wa_price": 20.0, "mod_rank": 0 },
            { "datetime": now.to_rfc3339(), "volume": 1, "wa_price": 80.0, "mod_rank": 10 }
        ]}}});
        assert_eq!(extract_average(&json, Variant::Rank(10), now).unwrap().0, 80.0);
        assert_eq!(extract_average(&json, Variant::Base, now).unwrap().0, 20.0);
    }
}
