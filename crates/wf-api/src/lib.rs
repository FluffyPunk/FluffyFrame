//! Library form of [wf-api-rust](https://github.com/FluffyPunk/wf-api-rust)
//! (itself a Rust port of Sainan's `warframe-api-helper`).
//!
//! It finds the running Warframe process, scans its memory for the
//! `?accountId=…&nonce=…` auth string the game uses for its own API calls,
//! and downloads the account inventory with it.
//!
//! `process.rs` is copied verbatim from wf-api-rust. The scan in [`find_authz`]
//! is the same algorithm as its `gruzzleAuthz`, with the console output
//! replaced by a typed [`ApiError`] whose [`ApiError::exit_code`] matches the
//! original tool's exit codes, so callers can tell the failure modes apart.

mod process;

use std::fmt;
use std::time::Duration;

pub use process::{ProcessHandle, Region, find_pid};

/// Process names to look for. Linux truncates names to 15 characters.
pub const PROCESS_NAMES: [&str; 2] = ["Warframe.x64.exe", "Warframe.x64.ex"];

const PATTERN: &[u8; 11] = b"?accountId=";
const ACCOUNT_ID_LEN: usize = 24;
const NONCE_PREFIX: &[u8; 7] = b"&nonce=";
/// See wf-api-rust: a single structurally valid occurrence is enough.
const REQUIRED_CONFIRMATIONS: u32 = 1;
/// Inventory hosts, tried in order.
const INVENTORY_HOSTS: [&str; 2] = ["mobile.warframe.com", "api.warframe.com"];
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Failure modes, mirroring wf-api-rust's exit codes.
#[derive(Debug)]
pub enum ApiError {
    /// Exit 1: Warframe is not running.
    GameNotRunning,
    /// Exit 2: the process could not be opened (e.g. game runs elevated).
    AccessDenied(std::io::Error),
    /// Exit 3: no auth string in memory (login screen, or scan missed it).
    TokenNotFound,
    /// Exit 4: process opened, but memory reads are denied (Linux ptrace_scope).
    ReadDenied(std::io::Error),
    /// Exit 5: the inventory request failed.
    Request(String),
    /// Exit 6: the server answered with something that is not JSON.
    InvalidResponse(String),
}

impl ApiError {
    pub fn exit_code(&self) -> u8 {
        match self {
            ApiError::GameNotRunning => 1,
            ApiError::AccessDenied(_) => 2,
            ApiError::TokenNotFound => 3,
            ApiError::ReadDenied(_) => 4,
            ApiError::Request(_) => 5,
            ApiError::InvalidResponse(_) => 6,
        }
    }

    /// Errors that resolve on their own within minutes (game starting, logging in).
    pub fn is_local_transient(&self) -> bool {
        matches!(self, ApiError::GameNotRunning | ApiError::TokenNotFound)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiError::GameNotRunning => write!(f, "Warframe is not running"),
            ApiError::AccessDenied(e) => write!(f, "could not open the Warframe process ({e})"),
            ApiError::TokenNotFound => write!(f, "login token not found in game memory"),
            ApiError::ReadDenied(e) => {
                write!(f, "reading game memory was denied ({e})")?;
                #[cfg(unix)]
                write!(
                    f,
                    ". On Linux this is usually Yama ptrace_scope: run \
                     `sudo setcap cap_sys_ptrace+ep <path to wfhelper>` or \
                     `sudo sysctl kernel.yama.ptrace_scope=0`"
                )?;
                Ok(())
            }
            ApiError::Request(e) => write!(f, "inventory request failed: {e}"),
            ApiError::InvalidResponse(e) => write!(f, "invalid inventory response: {e}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// The `?accountId=<24 hex>&nonce=<digits>` query string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authz(pub String);

impl Authz {
    pub fn account_id(&self) -> Option<&str> {
        self.0
            .strip_prefix("?accountId=")
            .and_then(|rest| rest.get(..ACCOUNT_ID_LEN))
    }
}

fn is_plausible_account_id(bytes: &[u8]) -> bool {
    bytes.len() == ACCOUNT_ID_LEN && bytes.iter().all(u8::is_ascii_hexdigit)
}

/// The PID of the running game, if any.
pub fn game_pid() -> Option<u32> {
    find_pid(&PROCESS_NAMES)
}

/// Opens the game process and verifies its memory is readable.
pub fn open_game() -> Result<ProcessHandle, ApiError> {
    let pid = game_pid().ok_or(ApiError::GameNotRunning)?;
    let mut handle = ProcessHandle::open(pid).map_err(ApiError::AccessDenied)?;
    handle.probe_read().map_err(ApiError::ReadDenied)?;
    Ok(handle)
}

/// Scans every readable region for the auth string (wf-api-rust's `gruzzleAuthz`).
pub fn find_authz(handle: &mut ProcessHandle) -> Result<Authz, ApiError> {
    let mut candidates: std::collections::HashMap<String, u32> = Default::default();
    for region in handle.regions() {
        let Some(data) = handle.read_vec(region.base, region.size) else {
            continue;
        };
        for match_offset in memchr::memmem::find_iter(&data, PATTERN) {
            let account_id_addr = region.base + match_offset + PATTERN.len();
            let Some(account_id_bytes) = handle.read_vec(account_id_addr, ACCOUNT_ID_LEN) else {
                continue;
            };
            if !is_plausible_account_id(&account_id_bytes) {
                continue;
            }
            let account_id = String::from_utf8_lossy(&account_id_bytes);

            let mut authz = format!("?accountId={account_id}&nonce=");
            let mut addr = account_id_addr + ACCOUNT_ID_LEN + NONCE_PREFIX.len();
            let mut nonce_digits = 0u32;
            while let Some(c) = handle.read_u8(addr) {
                addr += 1;
                if c.is_ascii_digit() {
                    authz.push(c as char);
                    nonce_digits += 1;
                } else {
                    break;
                }
            }
            if nonce_digits == 0 {
                continue;
            }
            let count = candidates.entry(authz.clone()).or_insert(0);
            *count += 1;
            if *count == REQUIRED_CONFIRMATIONS {
                return Ok(Authz(authz));
            }
        }
    }
    Err(ApiError::TokenNotFound)
}

/// Downloads `inventory.php` with the given auth string. Returns the raw JSON text,
/// validated to parse.
pub fn download_inventory(authz: &Authz) -> Result<String, ApiError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| ApiError::Request(e.to_string()))?;
    let mut last_err = String::new();
    for host in INVENTORY_HOSTS {
        let url = format!("https://{host}/api/inventory.php{}", authz.0);
        match client.get(&url).send().and_then(|r| r.error_for_status()) {
            Ok(response) => {
                let text = response.text().map_err(|e| ApiError::Request(e.to_string()))?;
                serde_json::from_str::<serde_json::Value>(&text)
                    .map_err(|e| ApiError::InvalidResponse(e.to_string()))?;
                return Ok(text);
            }
            Err(e) => last_err = e.to_string(),
        }
    }
    Err(ApiError::Request(last_err))
}

/// The whole pipeline: find the game, scan for the token, download the inventory.
pub fn fetch_inventory() -> Result<String, ApiError> {
    let mut handle = open_game()?;
    let authz = find_authz(&mut handle)?;
    download_inventory(&authz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plausible_account_id_accepts_24_hex_chars() {
        assert!(is_plausible_account_id(b"5a4766b43ade7f7aa7dfabc5"));
    }

    #[test]
    fn plausible_account_id_rejects_template_string_leftovers() {
        assert!(!is_plausible_account_id(b"&squadCount={\"nonce\":\"%s"));
    }

    #[test]
    fn find_authz_succeeds_on_a_single_valid_occurrence() {
        #[used]
        static FAKE_BUFFER: &[u8] =
            b"noise before ?accountId=0123456789abcdef01234567&nonce=123456789 trailing noise";

        let pid = std::process::id();
        let mut handle = ProcessHandle::open(pid).expect("should open own process");
        let authz = find_authz(&mut handle).expect("should find the embedded candidate");
        // Expected values are assembled at runtime: a full token literal here would
        // itself be a scan hit (and the linker may glue digits onto it).
        let id = ["0123456789abcdef", "01234567"].concat();
        let expected = format!("?account{}={id}&nonce={}", "Id", 123_456_789);
        assert_eq!(authz.0, expected);
        assert_eq!(authz.account_id(), Some(id.as_str()));
    }
}
