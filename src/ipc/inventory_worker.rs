//! Inventory loading: snapshot, game reads and imports (original: `ipc/inventoryIpc.ts`,
//! `services/inventorySync.ts`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use chrono::Utc;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use fluffyframe_core::config::settings::InventorySource;
use fluffyframe_core::services::api_helper_runner::{self, HelperStatus};
use fluffyframe_core::services::inventory_import;

use super::{Emitter, Event, Shared, ToastKind};
use crate::i18n::{tr, trf};

pub enum InventoryCmd {
    /// Read from the game now.
    RefreshNow,
    /// EE.log saw a login: read shortly after.
    LoginDetected,
    /// EE.log saw something that changes the inventory (mission end, trade,
    /// purchase, the game re-syncing it); the reason is for the log. Bursts
    /// collapse into one read: see [`SETTLE`] and [`MAX_DEFER`].
    GameChanged(&'static str),
    Import(PathBuf),
    Export(PathBuf),
    /// The source setting changed.
    SourceChanged,
}

/// Wait after a login before scanning, so the token is in memory.
const AFTER_LOGIN: Duration = Duration::from_secs(15);
/// After a change, wait this long (from the latest one) for the game to save
/// and for follow-up events: mission end, then the sync, arrive together.
const SETTLE: Duration = Duration::from_secs(20);
/// A steady stream of changes defers the read at most this long past the first.
const MAX_DEFER: Duration = Duration::from_secs(90);

pub fn run(shared: Arc<Shared>, emitter: Emitter, rx: Receiver<InventoryCmd>) {
    if let Some((inv, meta)) = inventory_import::load_snapshot() {
        log::info!("loaded inventory snapshot from {}", meta.fetched_at);
        emitter.emit(Event::Inventory(Arc::new(inv)));
    }
    let mut status = HelperStatus::default();
    // First game read soon after startup.
    let mut next_run = Instant::now() + Duration::from_secs(3);
    // First change since the last read, while a change-triggered read is pending.
    let mut changed_since: Option<Instant> = None;
    loop {
        let wait = next_run.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(InventoryCmd::RefreshNow) => next_run = Instant::now(),
            Ok(InventoryCmd::LoginDetected) => {
                if status.last_ok != Some(true) || status.last_run.is_none_or(|t| Utc::now() - t > chrono::Duration::minutes(2)) {
                    next_run = Instant::now() + AFTER_LOGIN;
                }
            }
            Ok(InventoryCmd::GameChanged(reason)) => {
                next_run = after_change(Instant::now(), &mut changed_since, next_run);
                log::info!("inventory refresh queued: {reason}");
            }
            Ok(InventoryCmd::SourceChanged) => next_run = Instant::now() + Duration::from_secs(1),
            Ok(InventoryCmd::Import(path)) => {
                match inventory_import::import_file(&path) {
                    Ok((inv, source)) => {
                        shared.settings.write().unwrap().inventory_source = source;
                        let _ = shared.settings.read().unwrap().save();
                        emitter.toast(ToastKind::Success, trf("Imported {}", &[&tr(source.label()).to_lowercase()]));
                        emitter.emit(Event::Inventory(Arc::new(inv)));
                    }
                    Err(e) => emitter.toast(ToastKind::Error, trf("Import failed: {}", &[&e])),
                }
                continue;
            }
            Ok(InventoryCmd::Export(path)) => {
                match inventory_import::export_to(&path) {
                    Ok(()) => emitter.toast(ToastKind::Success, trf("Exported to {}", &[&path.display()])),
                    Err(e) => emitter.toast(ToastKind::Error, trf("Export failed: {}", &[&e])),
                }
                continue;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if Instant::now() < next_run {
            continue;
        }

        let (source, interval) = {
            let s = shared.settings.read().unwrap();
            (s.inventory_source, Duration::from_secs(u64::from(s.helper_poll_minutes.max(1)) * 60))
        };
        if source != InventorySource::Game {
            // Imported inventories are never replaced by game reads.
            next_run = Instant::now() + Duration::from_secs(3600);
            continue;
        }

        changed_since = None;
        status.running = true;
        emitter.emit(Event::Helper(status.clone()));
        let logged_in = shared.logged_in.load(Ordering::Relaxed);
        let result = api_helper_runner::run_once(logged_in);
        status.running = false;
        status.last_run = Some(Utc::now());
        let outcome = match result {
            Ok(inv) => {
                status.last_ok = Some(true);
                status.last_failure = None;
                status.last_error = None;
                emitter.emit(Event::Inventory(Arc::new(inv)));
                emitter.toast(ToastKind::Info, tr("Inventory updated from the game"));
                Ok(())
            }
            Err((failure, message)) => {
                log::info!("game read failed: {message}");
                status.last_ok = Some(false);
                status.last_failure = Some(failure);
                status.last_error = Some(message);
                Err(failure)
            }
        };
        emitter.emit(Event::Helper(status.clone()));
        next_run = Instant::now() + api_helper_runner::next_delay(outcome, interval);
    }
}

/// When to read after a change at `now`. The first change only ever brings the
/// scheduled read forward; later ones push it to [`SETTLE`] after the latest,
/// capped at [`MAX_DEFER`] after the first.
fn after_change(now: Instant, changed_since: &mut Option<Instant>, next_run: Instant) -> Instant {
    let pending = changed_since.is_some();
    let first = *changed_since.get_or_insert(now);
    let target = (now + SETTLE).min(first + MAX_DEFER);
    if pending { target } else { next_run.min(target) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_collapse_into_one_read() {
        let t0 = Instant::now();
        let poll = t0 + Duration::from_secs(600);
        let mut since = None;
        // First change: read SETTLE later, well before the next poll.
        let next = after_change(t0, &mut since, poll);
        assert_eq!(next, t0 + SETTLE);
        // A follow-up 5s later moves the read to SETTLE after it.
        let t1 = t0 + Duration::from_secs(5);
        assert_eq!(after_change(t1, &mut since, next), t1 + SETTLE);
        // A stream of changes stops deferring at MAX_DEFER after the first.
        let t2 = t0 + Duration::from_secs(85);
        assert_eq!(after_change(t2, &mut since, t1 + SETTLE), t0 + MAX_DEFER);
    }

    #[test]
    fn a_change_never_delays_an_earlier_read() {
        let t0 = Instant::now();
        let soon = t0 + Duration::from_secs(5);
        let mut since = None;
        assert_eq!(after_change(t0, &mut since, soon), soon);
    }
}
