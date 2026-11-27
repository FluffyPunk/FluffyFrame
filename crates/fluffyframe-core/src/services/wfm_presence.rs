//! warframe.market presence (original: `services/wfmPresence.ts`,
//! `wfmWebSocket.ts`). The status is set over the site's WebSocket; the REST API
//! has no route for it. The current one is read from the public profile.

use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::Value;
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::wfm_client::{self, Api};

const WS_URL: &str = "wss://ws.warframe.market/socket";
const TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WfmStatus {
    Online,
    InGame,
    Invisible,
}

impl WfmStatus {
    pub const ALL: [WfmStatus; 3] = [WfmStatus::Online, WfmStatus::InGame, WfmStatus::Invisible];

    pub fn label(self) -> &'static str {
        match self {
            WfmStatus::Online => "Online",
            WfmStatus::InGame => "In game",
            WfmStatus::Invisible => "Invisible",
        }
    }

    fn api(self) -> &'static str {
        match self {
            WfmStatus::Online => "online",
            WfmStatus::InGame => "ingame",
            WfmStatus::Invisible => "invisible",
        }
    }

    /// The profile reports a hidden user as "offline".
    fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "online" => Some(WfmStatus::Online),
            "ingame" => Some(WfmStatus::InGame),
            "invisible" | "offline" => Some(WfmStatus::Invisible),
            _ => None,
        }
    }
}

/// The status others see on `profile` (the user's slug).
pub fn current(profile: &str) -> crate::Result<Option<WfmStatus>> {
    let json = wfm_client::request(Api::V2, reqwest::Method::GET, &format!("/user/{profile}"), None, true)?;
    Ok(json.pointer("/data/status").and_then(Value::as_str).and_then(WfmStatus::parse))
}

/// A WebSocket message id: 11 random letters and digits, like the site's.
fn message_id() -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    // Only needs to differ between messages: xorshift over the clock.
    let mut x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64) | 1;
    (0..11)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            CHARS[(x % CHARS.len() as u64) as usize] as char
        })
        .collect()
}

fn send(socket: &mut WebSocket<MaybeTlsStream<TcpStream>>, route: &str, payload: Value) -> crate::Result<()> {
    let msg = serde_json::json!({ "route": route, "payload": payload, "id": message_id() });
    socket.send(Message::text(msg.to_string()))?;
    Ok(())
}

/// Signs in on the socket with `token` and sets the status; returns once the
/// site confirms it.
pub fn set(token: &str, status: WfmStatus) -> crate::Result<()> {
    let mut request = WS_URL.into_client_request()?;
    let headers = request.headers_mut();
    headers.insert("Sec-WebSocket-Protocol", "wfm".parse()?);
    headers.insert("Origin", "https://warframe.market".parse()?);
    // Without one the site's proxy refuses the upgrade (403).
    headers.insert("User-Agent", crate::config::app_meta::user_agent().parse()?);
    let (mut socket, _) = tungstenite::connect(request)?;
    // A read timeout, so a silent server can't hang the worker.
    match socket.get_ref() {
        MaybeTlsStream::Rustls(s) => s.get_ref().set_read_timeout(Some(TIMEOUT))?,
        MaybeTlsStream::Plain(s) => s.set_read_timeout(Some(TIMEOUT))?,
        _ => {}
    }
    send(&mut socket, "@wfm|cmd/auth/signIn", serde_json::json!({ "token": token }))?;
    let deadline = Instant::now() + TIMEOUT;
    let result = loop {
        if Instant::now() > deadline {
            break Err(anyhow::anyhow!("warframe.market didn't confirm the status"));
        }
        let Message::Text(text) = socket.read()? else { continue };
        let msg: Value = serde_json::from_str(&text).unwrap_or_default();
        let route = msg.get("route").and_then(Value::as_str).unwrap_or("");
        if route.ends_with(":error") {
            break Err(anyhow::anyhow!("warframe.market: {route} {}", msg.get("payload").unwrap_or(&Value::Null)));
        }
        if route.contains("auth/signIn:ok") {
            send(&mut socket, "@wfm|cmd/status/set", serde_json::json!({ "status": status.api(), "duration": null }))?;
        } else if route.contains("status/set:ok") {
            break Ok(());
        }
    };
    let _ = socket.close(None);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_round_trip() {
        for s in WfmStatus::ALL {
            assert_eq!(WfmStatus::parse(s.api()), Some(s));
        }
        assert_eq!(WfmStatus::parse("offline"), Some(WfmStatus::Invisible));
        assert_eq!(WfmStatus::parse("away"), None);
        let id = message_id();
        assert!(id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}


