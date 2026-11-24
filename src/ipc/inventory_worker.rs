//! Inventory loading: snapshot, game reads and imports (original: `ipc/inventoryIpc.ts`,
//! `services/inventorySync.ts`).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use chrono::Utc;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use wfhelper_core::config::settings::InventorySource;
use wfhelper_core::services::api_helper_runner::{self, HelperStatus};
use wfhelper_core::services::inventory_import;

use super::{Emitter, Event, Shared, ToastKind};

pub enum InventoryCmd {
    /// Read from the game now.
    RefreshNow,
    /// EE.log saw a login: read shortly after.
    LoginDetected,
    Import(PathBuf),
    Export(PathBuf),
    /// The source setting changed.
    SourceChanged,
}

/// Wait after a login before scanning, so the token is in memory.
const AFTER_LOGIN: Duration = Duration::from_secs(15);

pub fn run(shared: Arc<Shared>, emitter: Emitter, rx: Receiver<InventoryCmd>) {
    if let Some((inv, meta)) = inventory_import::load_snapshot() {
        log::info!("loaded inventory snapshot from {}", meta.fetched_at);
        emitter.emit(Event::Inventory(Arc::new(inv)));
    }
    let mut status = HelperStatus::default();
    // First game read soon after startup.
    let mut next_run = Instant::now() + Duration::from_secs(3);
    loop {
        let wait = next_run.saturating_duration_since(Instant::now());
        match rx.recv_timeout(wait) {
            Ok(InventoryCmd::RefreshNow) => next_run = Instant::now(),
            Ok(InventoryCmd::LoginDetected) => {
                if status.last_ok != Some(true) || status.last_run.is_none_or(|t| Utc::now() - t > chrono::Duration::minutes(2)) {
                    next_run = Instant::now() + AFTER_LOGIN;
                }
            }
            Ok(InventoryCmd::SourceChanged) => next_run = Instant::now() + Duration::from_secs(1),
            Ok(InventoryCmd::Import(path)) => {
                match inventory_import::import_file(&path) {
                    Ok((inv, source)) => {
                        shared.settings.write().unwrap().inventory_source = source;
                        let _ = shared.settings.read().unwrap().save();
                        emitter.toast(ToastKind::Success, format!("Imported {}", source.label().to_lowercase()));
                        emitter.emit(Event::Inventory(Arc::new(inv)));
                    }
                    Err(e) => emitter.toast(ToastKind::Error, format!("Import failed: {e}")),
                }
                continue;
            }
            Ok(InventoryCmd::Export(path)) => {
                match inventory_import::export_to(&path) {
                    Ok(()) => emitter.toast(ToastKind::Success, format!("Exported to {}", path.display())),
                    Err(e) => emitter.toast(ToastKind::Error, format!("Export failed: {e}")),
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
                emitter.toast(ToastKind::Info, "Inventory updated from the game");
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
