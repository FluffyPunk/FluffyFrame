//! Arbitration schedule (original: `services/arbiSchedule.ts`).
//!
//! browse.wf publishes the precomputed hourly rotation as `unix_seconds,NodeId` lines.
//! The EE.log run analysis and the arbitration summary overlay are not ported.

use chrono::{DateTime, TimeZone, Utc};

use super::{http, json_cache};
use crate::config::constants::{ARBI_SCHEDULE_TTL, ARBITRATION_SCHEDULE_URL};

#[derive(Debug, Clone, PartialEq)]
pub struct ArbiSlot {
    pub start: DateTime<Utc>,
    pub node_id: String,
}

impl ArbiSlot {
    pub fn end(&self) -> DateTime<Utc> {
        self.start + chrono::Duration::hours(1)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ArbiSchedule {
    pub slots: Vec<ArbiSlot>,
}

impl ArbiSchedule {
    pub fn parse(text: &str) -> Self {
        let mut slots: Vec<ArbiSlot> = text
            .lines()
            .filter_map(|l| {
                let (ts, node) = l.trim().split_once(',')?;
                Some(ArbiSlot {
                    start: Utc.timestamp_opt(ts.parse().ok()?, 0).single()?,
                    node_id: node.trim().to_string(),
                })
            })
            .collect();
        slots.sort_by_key(|s| s.start);
        Self { slots }
    }

    pub fn load() -> crate::Result<Self> {
        let text = json_cache::fetch_cached("arbys.txt", ARBI_SCHEDULE_TTL, || {
            http::get_text(ARBITRATION_SCHEDULE_URL)
        })?;
        Ok(Self::parse(&text))
    }

    pub fn current(&self, now: DateTime<Utc>) -> Option<&ArbiSlot> {
        let idx = self.slots.partition_point(|s| s.start <= now);
        idx.checked_sub(1).map(|i| &self.slots[i]).filter(|s| s.end() > now)
    }

    /// Slots starting after `now`, up to `limit`.
    pub fn upcoming(&self, now: DateTime<Utc>, limit: usize) -> &[ArbiSlot] {
        let idx = self.slots.partition_point(|s| s.start <= now);
        let end = (idx + limit).min(self.slots.len());
        &self.slots[idx..end]
    }

    /// Next slot on any of `nodes`.
    pub fn next_on<'a>(&'a self, now: DateTime<Utc>, nodes: &[String]) -> Option<&'a ArbiSlot> {
        let idx = self.slots.partition_point(|s| s.start <= now);
        self.slots[idx..].iter().find(|s| nodes.contains(&s.node_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_and_upcoming() {
        let s = ArbiSchedule::parse("3600,SolNode2\n0,SolNode1\n7200,SolNode3\n");
        let now = Utc.timestamp_opt(4000, 0).unwrap();
        assert_eq!(s.current(now).unwrap().node_id, "SolNode2");
        assert_eq!(s.upcoming(now, 5)[0].node_id, "SolNode3");
        assert_eq!(s.next_on(now, &["SolNode3".into()]).unwrap().start.timestamp(), 7200);
        assert!(s.current(Utc.timestamp_opt(99_999, 0).unwrap()).is_none());
    }
}
