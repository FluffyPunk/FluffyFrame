//! Desktop notifications (original: `services/notificationChannels.ts`,
//! `notificationLog.ts`). Sent off the UI thread; failures are logged, not shown.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Duration, Utc};

use super::world_state::Cycle;
use crate::config::app_meta::APP_NAME;

pub fn send(title: &str, body: &str) {
    log::info!("notification: {title}: {body}");
    let (title, body) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        if let Err(e) = notify_rust::Notification::new()
            .appname(APP_NAME)
            .summary(&title)
            .body(&body)
            .show()
        {
            log::warn!("notification failed: {e}");
        }
    });
}

/// Decides planet-cycle alerts (original: `checkPreCycleNotifications` and the
/// cycle transitions in `worldStateIpc.ts`).
#[derive(Default)]
pub struct CycleWatch {
    /// Each cycle's state at the last check; empty until the first one.
    states: HashMap<&'static str, &'static str>,
    /// Cycle changes already warned about ahead of time.
    warned: HashSet<(&'static str, DateTime<Utc>)>,
    /// When the last check ran.
    last: Option<DateTime<Utc>>,
}

/// A planet-cycle alert; the app words it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleAlert {
    /// `cycle` changed to `state`.
    Began { cycle: &'static str, state: &'static str },
    /// `cycle` changes to `next` in `minutes`.
    Ahead { cycle: &'static str, next: &'static str, minutes: i64 },
}

impl CycleWatch {
    /// Alerts for each `enabled` cycle that changed since the last check, or
    /// changes within `lead`. The first check only records states, so there is
    /// no burst of alerts at startup.
    pub fn check(&mut self, cycles: &[Cycle], enabled: &[String], lead: Duration, now: DateTime<Utc>) -> Vec<CycleAlert> {
        let primed = !self.states.is_empty();
        let mut out = Vec::new();
        for c in cycles {
            let previous = self.states.insert(c.name, c.state);
            if !enabled.iter().any(|e| e == c.name) {
                continue;
            }
            if primed && previous.is_some_and(|p| p != c.state) {
                out.push(CycleAlert::Began { cycle: c.name, state: c.state });
            }
            let left = c.ends_at - now;
            // The first warning comes as soon as the change is within `lead`.
            // After that, only when the countdown crosses the lead since the last
            // check: a lead changed later warns again at its own time, but
            // values passed through while editing it don't.
            let crossed = self.last.is_some_and(|last| c.ends_at - last > lead);
            let first = !self.warned.contains(&(c.name, c.ends_at));
            if lead > Duration::zero() && left <= lead && (first || crossed) {
                self.warned.insert((c.name, c.ends_at));
                let minutes = (left.num_seconds() + 59) / 60;
                out.push(CycleAlert::Ahead { cycle: c.name, next: c.next, minutes });
            }
        }
        self.warned.retain(|(_, end)| *end > now);
        self.last = Some(now);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn earth(state: &'static str, next: &'static str, ends_at: DateTime<Utc>) -> Cycle {
        Cycle { name: "Earth", state, next, ends_at }
    }

    #[test]
    fn cycle_alerts_on_change_and_ahead() {
        let t = Utc.with_ymd_and_hms(2026, 11, 27, 0, 0, 0).unwrap();
        let end = t + Duration::minutes(10);
        let on = vec!["Earth".to_string()];
        let lead = Duration::minutes(3);
        let mut w = CycleWatch::default();
        // First check only primes, even inside the lead window.
        assert!(w.check(&[earth("Day", "Night", end)], &on, lead, t).is_empty());
        assert!(w.check(&[earth("Day", "Night", end)], &on, lead, t + Duration::minutes(6)).is_empty());
        let ahead = w.check(&[earth("Day", "Night", end)], &on, lead, t + Duration::minutes(8));
        assert_eq!(ahead, vec![CycleAlert::Ahead { cycle: "Earth", next: "Night", minutes: 2 }]);
        // Warned once…
        let at = |s: i64| t + Duration::seconds(s);
        assert!(w.check(&[earth("Day", "Night", end)], &on, lead, at(490)).is_empty());
        // …and editing the lead through values already inside the window stays quiet…
        for (s, m) in [(500, 2), (510, 1)] {
            assert!(w.check(&[earth("Day", "Night", end)], &on, Duration::minutes(m), at(s)).is_empty());
        }
        // …until the countdown crosses the new lead.
        let short = Duration::minutes(1);
        assert!(w.check(&[earth("Day", "Night", end)], &on, short, at(535)).is_empty());
        let again = w.check(&[earth("Day", "Night", end)], &on, short, at(545));
        assert_eq!(again, vec![CycleAlert::Ahead { cycle: "Earth", next: "Night", minutes: 1 }]);
        assert!(w.check(&[earth("Day", "Night", end)], &on, short, at(555)).is_empty());
        // A bell turned on inside the window still warns right away.
        let mut late = CycleWatch::default();
        late.check(&[earth("Day", "Night", end)], &[], lead, at(500));
        assert_eq!(late.check(&[earth("Day", "Night", end)], &on, lead, at(510)).len(), 1);
        let next_end = end + Duration::hours(4);
        let changed = w.check(&[earth("Night", "Day", next_end)], &on, lead, end);
        assert_eq!(changed, vec![CycleAlert::Began { cycle: "Earth", state: "Night" }]);
        // Disabled cycles stay quiet but are still tracked.
        assert!(w.check(&[earth("Day", "Night", next_end + Duration::hours(4))], &[], lead, next_end).is_empty());
    }
}
