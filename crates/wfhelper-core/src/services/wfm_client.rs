//! warframe.market HTTP client (original: `services/wfmClient.ts`, `wfmSession.ts`).
//!
//! Every request goes through one process-wide gate that keeps ~3 req/s and backs
//! off after a 429, so prices, orders and riven searches never trip the ban.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{atomic_file, http};
use crate::config::constants::{WFM_MIN_REQUEST_GAP, WFM_RATE_LIMIT_COOLDOWN, WFM_SITE, WFM_V1, WFM_V2};
use crate::config::paths;

struct Gate {
    next_allowed: Instant,
}

static GATE: Mutex<Option<Gate>> = Mutex::new(None);
static SESSION: Mutex<Option<Session>> = Mutex::new(None);

/// Signed-in state. The JWT is stored in the data directory, never logged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub ingame_name: Option<String>,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Api {
    V1,
    V2,
}

fn wait_turn() {
    loop {
        let wait = {
            let mut guard = GATE.lock().unwrap();
            let gate = guard.get_or_insert(Gate { next_allowed: Instant::now() });
            let now = Instant::now();
            if now >= gate.next_allowed {
                gate.next_allowed = now + WFM_MIN_REQUEST_GAP;
                None
            } else {
                Some(gate.next_allowed - now)
            }
        };
        match wait {
            None => return,
            Some(d) => std::thread::sleep(d),
        }
    }
}

fn cool_down(d: Duration) {
    let mut guard = GATE.lock().unwrap();
    let gate = guard.get_or_insert(Gate { next_allowed: Instant::now() });
    gate.next_allowed = Instant::now() + d;
}

pub fn session() -> Option<Session> {
    let mut guard = SESSION.lock().unwrap();
    if guard.is_none() {
        *guard = atomic_file::read_json(&paths::wfm_session_file());
    }
    guard.clone()
}

fn set_session(session: Option<Session>) {
    let path = paths::wfm_session_file();
    match &session {
        Some(s) => {
            if let Err(e) = atomic_file::write_json(&path, s) {
                log::warn!("could not persist warframe.market session: {e}");
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
        None => {
            let _ = std::fs::remove_file(&path);
        }
    }
    *SESSION.lock().unwrap() = session;
}

/// Sends a request. `auth` attaches the session token when signed in.
pub fn request(api: Api, method: Method, path: &str, body: Option<&Value>, auth: bool) -> crate::Result<Value> {
    let base = match api {
        Api::V1 => WFM_V1,
        Api::V2 => WFM_V2,
    };
    let url = format!("{base}{path}");
    for attempt in 0..3 {
        wait_turn();
        let mut req = http::client()
            .request(method.clone(), &url)
            .header("Platform", "pc")
            .header("Language", "en")
            .header("Accept", "application/json")
            .header("Crossplay", "true");
        if auth && let Some(s) = session() {
            // v2 takes a Bearer token, v1 the legacy JWT scheme.
            let scheme = match api {
                Api::V1 => "JWT",
                Api::V2 => "Bearer",
            };
            req = req.header("Authorization", format!("{scheme} {}", s.token));
        }
        if let Some(body) = body {
            req = req.json(body);
        }
        let resp = req.send().with_context(|| format!("{method} {path}"))?;
        let status = resp.status();
        if status.as_u16() == 429 {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(Duration::from_secs)
                .unwrap_or(WFM_RATE_LIMIT_COOLDOWN);
            log::warn!("warframe.market rate limit, cooling down {retry:?}");
            cool_down(retry);
            if attempt < 2 {
                continue;
            }
        }
        if status.as_u16() == 401 && auth {
            set_session(None);
            bail!("warframe.market session expired, sign in again");
        }
        let text = resp.text()?;
        if !status.is_success() {
            bail!("warframe.market {status} for {path}: {}", text.chars().take(200).collect::<String>());
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        return serde_json::from_str(&text).with_context(|| format!("decoding {path}"));
    }
    bail!("warframe.market kept rate limiting {path}")
}

pub fn get(api: Api, path: &str) -> crate::Result<Value> {
    request(api, Method::GET, path, None, false)
}

/// v2 responses wrap their payload as `{"data": …, "error": …}`.
pub fn unwrap_v2(value: Value) -> crate::Result<Value> {
    if let Some(err) = value.get("error").filter(|e| !e.is_null()) {
        bail!("warframe.market error: {err}");
    }
    Ok(value.get("data").cloned().unwrap_or(value))
}

/// Email/password sign-in. The password is sent once and never stored.
pub fn sign_in(email: &str, password: &str) -> crate::Result<Session> {
    wait_turn();
    let resp = http::client()
        .post(format!("{WFM_V1}/auth/signin"))
        .header("Platform", "pc")
        .header("Language", "en")
        .header("Authorization", "JWT")
        .header("Origin", WFM_SITE)
        .header("Referer", format!("{WFM_SITE}/"))
        .json(&serde_json::json!({ "email": email, "password": password, "auth_type": "header" }))
        .send()?;
    let status = resp.status();
    let token = resp
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim_start_matches("JWT ").trim_start_matches("Bearer ").to_string());
    let body: Value = resp.json().unwrap_or(Value::Null);
    if !status.is_success() {
        let reason = body
            .pointer("/error/email")
            .or_else(|| body.pointer("/error/password"))
            .or_else(|| body.get("error"))
            .map(|e| e.to_string())
            .unwrap_or_else(|| status.to_string());
        bail!("sign-in failed: {reason}");
    }
    let token = token.filter(|t| !t.is_empty()).context("sign-in returned no token")?;
    let user = body.pointer("/payload/user");
    let session = Session {
        token,
        ingame_name: user.and_then(|u| u.get("ingame_name")).and_then(Value::as_str).map(str::to_string),
        slug: user.and_then(|u| u.get("slug")).and_then(Value::as_str).map(str::to_string),
    };
    set_session(Some(session.clone()));
    // Fill the profile from v2 if v1 left it out.
    if session.ingame_name.is_none()
        && let Ok(me) = request(Api::V2, Method::GET, "/me", None, true).and_then(unwrap_v2)
    {
        let mut s = session.clone();
        s.ingame_name = me.get("ingameName").and_then(Value::as_str).map(str::to_string);
        s.slug = me.get("slug").and_then(Value::as_str).map(str::to_string);
        set_session(Some(s.clone()));
        return Ok(s);
    }
    Ok(session)
}

pub fn sign_out() {
    set_session(None);
}
