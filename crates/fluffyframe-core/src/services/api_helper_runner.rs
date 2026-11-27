//! Automatic inventory reads from the running game (original:
//! `services/apiHelperRunner.ts`, `gameMemoryAuthz.ts`, `gameMemoryWin.ts`).
//!
//! The original downloaded Sainan's helper binary on Windows and scanned memory
//! in-process on Linux. Here both platforms use the `wf-api` crate (FluffyPunk's
//! wf-api-rust) directly, so there is nothing to download.

use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use wf_api::{ApiError, Authz};

use super::inventory::Inventory;
use super::inventory_import;
use crate::config::constants::HELPER_LOCAL_FAILURE_RETRY;
use crate::config::settings::InventorySource;

/// Why a run failed, in the original's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunFailure {
    GameNotRunning,
    AccessDenied,
    NotLoggedIn,
    TokenNotFound,
    ReadDenied,
    Network,
    InvalidResponse,
}

impl RunFailure {
    pub fn from_error(e: &ApiError, logged_in: bool) -> Self {
        match e {
            ApiError::GameNotRunning => RunFailure::GameNotRunning,
            ApiError::AccessDenied(_) => RunFailure::AccessDenied,
            // Exit 3 conflates the login screen with a failed scan; EE.log tells them apart.
            ApiError::TokenNotFound if logged_in => RunFailure::TokenNotFound,
            ApiError::TokenNotFound => RunFailure::NotLoggedIn,
            ApiError::ReadDenied(_) => RunFailure::ReadDenied,
            ApiError::Request(_) => RunFailure::Network,
            ApiError::InvalidResponse(_) => RunFailure::InvalidResponse,
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            RunFailure::GameNotRunning => "Waiting for the game: start Warframe and finish logging in.",
            RunFailure::AccessDenied => {
                "Access denied: Warframe may be running as administrator. Restart it without elevation."
            }
            RunFailure::NotLoggedIn => "Warframe is at the login screen. Log in and the inventory will follow.",
            RunFailure::TokenNotFound => "Login token not found: restart Warframe and try again.",
            RunFailure::ReadDenied => {
                "Reading game memory was denied. On Linux run `sudo setcap cap_sys_ptrace+ep <fluffyframe binary>` \
                 or `sudo sysctl kernel.yama.ptrace_scope=0`."
            }
            RunFailure::Network => "Could not reach DE's inventory API. It will retry.",
            RunFailure::InvalidResponse => "DE's inventory API returned an unexpected response.",
        }
    }

    /// Failures that resolve on their own within minutes retry sooner.
    pub fn retries_fast(self) -> bool {
        matches!(
            self,
            RunFailure::GameNotRunning | RunFailure::AccessDenied | RunFailure::NotLoggedIn
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct HelperStatus {
    pub running: bool,
    pub last_run: Option<chrono::DateTime<Utc>>,
    pub last_ok: Option<bool>,
    pub last_failure: Option<RunFailure>,
    pub last_error: Option<String>,
}

/// One full read: scan the game, download, persist the snapshot.
pub fn run_once(logged_in: bool) -> Result<Inventory, (RunFailure, String)> {
    let started = Instant::now();
    let text = wf_api::fetch_inventory().map_err(|e| (RunFailure::from_error(&e, logged_in), e.to_string()))?;
    finish(&text, started)
}

/// A read reusing the auth string an earlier one found (`authz`): scanning
/// the game's memory is most of a read, the download alone is quick. A stale
/// string (the game logged in again) fails the download or isn't answered
/// with an inventory; the memory is then scanned again and `authz` updated.
pub fn run_reusing(authz: &mut Option<Authz>, logged_in: bool) -> Result<Inventory, (RunFailure, String)> {
    let started = Instant::now();
    if let Some(known) = authz.as_ref()
        && let Ok(text) = wf_api::download_inventory(known)
        && let Ok(inv) = finish(&text, started)
    {
        return Ok(inv);
    }
    let err = |e: ApiError| (RunFailure::from_error(&e, logged_in), e.to_string());
    let found = wf_api::find_authz(&mut wf_api::open_game().map_err(err)?).map_err(err)?;
    let text = wf_api::download_inventory(&found).map_err(err)?;
    *authz = Some(found);
    finish(&text, started)
}

/// Parses a downloaded inventory and persists the snapshot.
fn finish(text: &str, started: Instant) -> Result<Inventory, (RunFailure, String)> {
    let mut inv = Inventory::parse_text(text).map_err(|e| (RunFailure::InvalidResponse, e.to_string()))?;
    if let Err(e) = inventory_import::save_snapshot(text, InventorySource::Game) {
        log::warn!("could not save inventory snapshot: {e}");
    }
    inv.source = Some(InventorySource::Game);
    inv.fetched_at = Some(Utc::now());
    log::info!("inventory read from game in {:.1}s", started.elapsed().as_secs_f32());
    Ok(inv)
}

/// Delay before the next scheduled run.
pub fn next_delay(result: Result<(), RunFailure>, interval: Duration) -> Duration {
    match result {
        Err(f) if f.retries_fast() => HELPER_LOCAL_FAILURE_RETRY.min(interval),
        _ => interval,
    }
}

/// Whether Warframe is running (its process exists, logged in or not).
pub fn game_running() -> bool {
    wf_api::game_pid().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_policy() {
        let ten = Duration::from_secs(600);
        assert_eq!(next_delay(Ok(()), ten), ten);
        assert_eq!(next_delay(Err(RunFailure::GameNotRunning), ten), HELPER_LOCAL_FAILURE_RETRY);
        assert_eq!(next_delay(Err(RunFailure::TokenNotFound), ten), ten);
    }

    #[test]
    fn token_classification() {
        assert_eq!(RunFailure::from_error(&ApiError::TokenNotFound, true), RunFailure::TokenNotFound);
        assert_eq!(RunFailure::from_error(&ApiError::TokenNotFound, false), RunFailure::NotLoggedIn);
    }
}
