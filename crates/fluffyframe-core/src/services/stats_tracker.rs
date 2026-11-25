//! Daily currency history (original: `services/statsTracker.ts`).
//!
//! Each inventory load stores that day's platinum, credits, endo and ducats; the
//! Stats view charts the per-day values and deltas.

use std::collections::BTreeMap;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::atomic_file;
use super::inventory::Currencies;
use crate::config::paths;

const MAX_DAYS: usize = 730;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct DayStats {
    pub platinum: u64,
    pub credits: u64,
    pub endo: u64,
    pub ducats: u64,
    #[serde(default)]
    pub aya: u64,
    #[serde(default)]
    pub vitus: u64,
    #[serde(default)]
    pub kuva: u64,
}

impl From<&Currencies> for DayStats {
    fn from(c: &Currencies) -> Self {
        Self {
            platinum: c.platinum,
            credits: c.credits,
            endo: c.endo,
            ducats: c.ducats,
            aya: c.aya,
            vitus: c.vitus,
            kuva: c.kuva,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StatsHistory {
    /// "YYYY-MM-DD" (local day) → last snapshot of that day.
    pub days: BTreeMap<String, DayStats>,
}

pub fn day_key(at: DateTime<Local>) -> String {
    at.format("%Y-%m-%d").to_string()
}

impl StatsHistory {
    pub fn load() -> Self {
        atomic_file::read_json(&paths::stats_file()).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Err(e) = atomic_file::write_json(&paths::stats_file(), self) {
            log::warn!("stats save failed: {e}");
        }
    }

    pub fn record(&mut self, currencies: &Currencies, at: DateTime<Local>) {
        self.days.insert(day_key(at), currencies.into());
        while self.days.len() > MAX_DAYS {
            let first = self.days.keys().next().cloned().unwrap();
            self.days.remove(&first);
        }
    }

    /// (day, value, change vs previous recorded day) for one field.
    pub fn series(&self, field: fn(&DayStats) -> u64) -> Vec<(String, u64, i64)> {
        let mut prev: Option<u64> = None;
        self.days
            .iter()
            .map(|(day, s)| {
                let v = field(s);
                let delta = prev.map(|p| v as i64 - p as i64).unwrap_or(0);
                prev = Some(v);
                (day.clone(), v, delta)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_deltas() {
        let mut h = StatsHistory::default();
        h.days.insert("2026-01-01".into(), DayStats { platinum: 100, ..Default::default() });
        h.days.insert("2026-01-02".into(), DayStats { platinum: 130, ..Default::default() });
        let s = h.series(|d| d.platinum);
        assert_eq!(s[1], ("2026-01-02".into(), 130, 30));
    }
}
