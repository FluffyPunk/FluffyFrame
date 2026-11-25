//! Persisted trade history from EE.log (original: `services/tradeTracker.ts`,
//! `tradeLedgerStore.ts`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::atomic_file;
use super::ee_log_monitor::{LogTrade, TradeKind};
use crate::config::paths;

const MAX_EVENTS: usize = 2000;
/// The file poll can deliver a dialog twice; identical trades this close are one.
const DUPLICATE_WINDOW_SECS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TradeEvent {
    pub at: DateTime<Utc>,
    #[serde(flatten)]
    pub trade: LogTrade,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TradeLog {
    pub events: Vec<TradeEvent>,
}

impl TradeLog {
    pub fn load() -> Self {
        atomic_file::read_json(&paths::trades_file()).unwrap_or_default()
    }

    pub fn save(&self) {
        if let Err(e) = atomic_file::write_json(&paths::trades_file(), self) {
            log::warn!("trade log save failed: {e}");
        }
    }

    /// Records a trade; returns false for a duplicate delivery.
    pub fn record(&mut self, trade: LogTrade, at: DateTime<Utc>) -> bool {
        let dup = self
            .events
            .iter()
            .rev()
            .take(5)
            .any(|e| e.trade == trade && (at - e.at).num_seconds().abs() < DUPLICATE_WINDOW_SECS);
        if dup {
            return false;
        }
        self.events.push(TradeEvent { at, trade });
        if self.events.len() > MAX_EVENTS {
            let drop = self.events.len() - MAX_EVENTS;
            self.events.drain(..drop);
        }
        true
    }

    /// (platinum earned, platinum spent) since `from`.
    pub fn plat_totals(&self, from: DateTime<Utc>) -> (u64, u64) {
        self.events.iter().filter(|e| e.at >= from).fold((0, 0), |(earned, spent), e| match e.trade.kind {
            TradeKind::Sale => (earned + e.trade.platinum as u64, spent),
            TradeKind::Purchase => (earned, spent + e.trade.platinum as u64),
            TradeKind::Trade => (earned, spent),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(plat: u32, kind: TradeKind) -> LogTrade {
        LogTrade { partner: "x".into(), kind, platinum: plat, items: vec![] }
    }

    #[test]
    fn dedupes_and_totals() {
        let mut log = TradeLog::default();
        let now = Utc::now();
        assert!(log.record(trade(10, TradeKind::Sale), now));
        assert!(!log.record(trade(10, TradeKind::Sale), now));
        assert!(log.record(trade(4, TradeKind::Purchase), now));
        assert_eq!(log.plat_totals(now - chrono::Duration::hours(1)), (10, 4));
    }
}
