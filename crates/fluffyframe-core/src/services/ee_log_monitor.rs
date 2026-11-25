//! EE.log tail (original: `services/eeLogMonitor.ts`, `tradeLogSanitize.ts`).
//!
//! Polls the log, follows truncation on game restart, and emits logins, whispers,
//! completed trades and mission ends. Reward-screen and relic-picker triggers fed
//! the overlays, which are not ported, so they are not detected here.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};

use super::ee_log_path;
use crate::config::constants::EE_LOG_POLL;

const MAX_READ_BYTES: u64 = 256 * 1024;
const CHAT_TAB_MARKER: &str = "ChatRedux::AddTab: Adding tab with channel name: ";
const TRADE_SUCCESS: &str = "The trade was successful!";
const OFFERING: &str = "You are offering:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradeKind {
    Sale,
    Purchase,
    Trade,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Given,
    Received,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeItem {
    pub name: String,
    pub count: u32,
    pub direction: Direction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogTrade {
    pub partner: String,
    pub kind: TradeKind,
    pub platinum: u32,
    pub items: Vec<TradeItem>,
}

#[derive(Debug, Clone)]
pub enum LogEvent {
    /// The file the monitor follows (or `None` when not found).
    Attached(Option<PathBuf>),
    /// The log was recreated: the game (re)started.
    GameStarted,
    LoginComplete,
    Whisper(String),
    TradeCompleted(LogTrade),
    MissionEnded,
    /// The game reloaded the inventory from DE's servers (`OnInventoryResults`):
    /// at login and after missions, the hub and other changes.
    InventorySynced,
    /// A market purchase went through (the purchase celebration screen).
    PurchaseCompleted,
}

/// Strips platform glyphs (PUA code points, U+FFFD and trailing non-ASCII).
pub fn strip_platform_glyphs(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .filter(|&c| !(('\u{E000}'..='\u{F8FF}').contains(&c) || c == '\u{FFFD}'))
        .collect();
    let trimmed = cleaned.trim_end_matches(|c: char| !c.is_ascii());
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A log framework line rather than an item ("11828.904 Script [Info]: Dialog.lua: …").
pub fn is_log_framework_line(value: &str) -> bool {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^\d+\.\d+\s|\[(Info|Error|Warning)\]|\b[A-Za-z_]+\.lua\b").unwrap())
        .is_match(value)
}

/// Cuts the `Dialog::CreateOkCancel` arg tail glued to the last item.
pub fn strip_dialog_arg_tail(value: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i),?\s*\b(?:title|leftItem|rightItem)=.*$").unwrap())
        .replace(value, "")
        .into_owned()
}

fn has_dialog_tail(value: &str) -> bool {
    value.contains("leftItem=") || value.contains("rightItem=")
}

/// Private chat tabs prefix the username with `F`.
pub fn parse_whisper_username(line: &str) -> Option<String> {
    let start = line.find(CHAT_TAB_MARKER)? + CHAT_TAB_MARKER.len();
    let rest = &line[start..];
    let name = &rest[..rest.find(" to index")?];
    let name = name.strip_prefix('F')?;
    let name = name.trim_end_matches(|c: char| !c.is_ascii()).trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Parses the buffered trade confirmation dialog.
pub fn parse_trade_dialog(lines: &[String]) -> Option<LogTrade> {
    let text = lines.join("\n");
    let desc = &text[text.find(OFFERING)?..];
    let re = regex::Regex::new(r"(?is)and will receive from\s+(.+?)\s+the following:").unwrap();
    let m = re.captures(desc)?;
    let divider = m.get(0)?;
    let partner = strip_platform_glyphs(m.get(1)?.as_str());
    let offering = &desc[OFFERING.len()..divider.start()];
    let receiving = &desc[divider.end()..];

    let plat_re = regex::Regex::new(r"(?i)^Platinum(?:\s+x\s+(\d+))?$").unwrap();
    let stack_re = regex::Regex::new(r"(?i)^(.+?)\s+x\s+(\d+)$").unwrap();
    let parse_block = |block: &str, direction: Direction| -> (Vec<TradeItem>, u32) {
        let mut plat = 0;
        let mut items: Vec<TradeItem> = Vec::new();
        for raw in block.split(['\n', '\r']) {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("leftItem=") || line.starts_with("rightItem=") || line.starts_with("title=") {
                break;
            }
            let without_tail = strip_dialog_arg_tail(line);
            if is_log_framework_line(&without_tail) {
                continue;
            }
            let cleaned = strip_platform_glyphs(&without_tail);
            if cleaned.is_empty() {
                continue;
            }
            if let Some(c) = plat_re.captures(&cleaned) {
                plat += c.get(1).and_then(|n| n.as_str().parse().ok()).unwrap_or(1);
                continue;
            }
            let (name, qty) = match stack_re.captures(&cleaned) {
                Some(c) => (c[1].trim().to_string(), c[2].parse().unwrap_or(1)),
                None => (cleaned.clone(), 1),
            };
            match items.iter_mut().find(|i| i.name == name) {
                Some(existing) => existing.count += qty,
                None => items.push(TradeItem { name, count: qty, direction }),
            }
        }
        (items, plat)
    };
    let (mut given, spent) = parse_block(offering, Direction::Given);
    let (received, gained) = parse_block(receiving, Direction::Received);
    let kind = match (gained > 0, spent > 0) {
        (true, false) => TradeKind::Sale,
        (false, true) => TradeKind::Purchase,
        _ => TradeKind::Trade,
    };
    given.extend(received);
    Some(LogTrade { partner, kind, platinum: gained.max(spent), items: given })
}

/// Line-level state machine, separate from file IO so it is testable.
#[derive(Default)]
pub struct LineParser {
    dialog: Vec<String>,
    dialog_open: bool,
    dialog_sealed: bool,
    last_dialog: Option<Vec<String>>,
}

impl LineParser {
    pub fn handle(&mut self, line: &str, out: &mut Vec<LogEvent>) {
        if line.contains("MainMenu::LoginDone result=true") || line.contains("Logging in as ") {
            out.push(LogEvent::LoginComplete);
        }
        if let Some(name) = parse_whisper_username(line) {
            out.push(LogEvent::Whisper(name));
        }
        if line.contains("Sys [Info]: EOM missionLocationUnlocked=") || line.contains("TopMenu.lua: Abort:") {
            out.push(LogEvent::MissionEnded);
        }
        if line.contains("Sys [Info]: OnInventoryResults completed") {
            out.push(LogEvent::InventorySynced);
        }
        if line.contains("Created /Lotus/Interface/PurchaseCelebration.swf") {
            out.push(LogEvent::PurchaseCompleted);
        }
        if line.contains(TRADE_SUCCESS) {
            let lines = if self.dialog_open { Some(std::mem::take(&mut self.dialog)) } else { self.last_dialog.take() };
            self.dialog_open = false;
            if let Some(trade) = lines.as_deref().and_then(parse_trade_dialog) {
                out.push(LogEvent::TradeCompleted(trade));
            }
            return;
        }
        if line.contains(OFFERING) {
            self.dialog = vec![line.to_string()];
            self.dialog_open = true;
            self.dialog_sealed = has_dialog_tail(line);
            if self.dialog_sealed {
                self.last_dialog = Some(std::mem::take(&mut self.dialog));
                self.dialog_open = false;
            }
            return;
        }
        if self.dialog_open && !self.dialog_sealed {
            for (i, segment) in line.split('\r').enumerate() {
                if is_log_framework_line(&strip_dialog_arg_tail(segment)) {
                    if i == 0 {
                        self.dialog_sealed = true;
                    }
                    break;
                }
                if !segment.trim().is_empty() {
                    self.dialog.push(segment.to_string());
                }
                if has_dialog_tail(segment) {
                    self.dialog_sealed = true;
                    break;
                }
            }
            if self.dialog_sealed {
                self.last_dialog = Some(std::mem::take(&mut self.dialog));
                self.dialog_open = false;
            }
        }
    }
}

pub struct MonitorHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MonitorHandle {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Starts following EE.log. History is skipped: only lines written from now on count.
pub fn spawn(override_path: Option<String>, tx: Sender<LogEvent>) -> MonitorHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("ee-log".into())
        .spawn(move || run(override_path, tx, stop_flag))
        .expect("spawn ee-log thread");
    MonitorHandle { stop, thread: Some(thread) }
}

fn run(override_path: Option<String>, tx: Sender<LogEvent>, stop: Arc<AtomicBool>) {
    let mut path: Option<PathBuf> = None;
    let mut offset = 0u64;
    let mut remainder = String::new();
    let mut parser = LineParser::default();
    let mut ticks_since_resolve = u32::MAX;
    while !stop.load(Ordering::Relaxed) {
        // Re-resolve every ~10 s: the newest log wins when several prefixes exist.
        if ticks_since_resolve >= 20 {
            ticks_since_resolve = 0;
            let found = ee_log_path::resolve(override_path.as_deref());
            if found != path {
                offset = found.as_ref().and_then(|p| std::fs::metadata(p).ok()).map(|m| m.len()).unwrap_or(0);
                remainder.clear();
                path = found.clone();
                log::info!("EE.log: {}", path.as_ref().map(|p| p.display().to_string()).unwrap_or("not found".into()));
                if tx.send(LogEvent::Attached(found)).is_err() {
                    return;
                }
            }
        }
        ticks_since_resolve += 1;

        if let Some(p) = &path
            && let Ok(mut file) = std::fs::File::open(p)
        {
            let len = file.metadata().map(|m| m.len()).unwrap_or(0);
            if len < offset {
                offset = 0;
                remainder.clear();
                let _ = tx.send(LogEvent::GameStarted);
            }
            let mut loops = 0;
            while offset < len && loops < 8 {
                loops += 1;
                let to_read = (len - offset).min(MAX_READ_BYTES);
                let mut buf = vec![0u8; to_read as usize];
                if file.seek(SeekFrom::Start(offset)).is_err() || file.read_exact(&mut buf).is_err() {
                    break;
                }
                offset += to_read;
                remainder.push_str(&String::from_utf8_lossy(&buf));
                let mut events = Vec::new();
                while let Some(nl) = remainder.find('\n') {
                    let line: String = remainder.drain(..=nl).collect();
                    parser.handle(line.trim_end_matches(['\n', '\r']), &mut events);
                }
                for e in events {
                    if tx.send(e).is_err() {
                        return;
                    }
                }
            }
        }
        std::thread::sleep(EE_LOG_POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper() {
        let line = "123.4 Script [Info]: ChatRedux::AddTab: Adding tab with channel name: FTennoGuy\u{e000} to index 4";
        assert_eq!(parse_whisper_username(line).as_deref(), Some("TennoGuy"));
        assert_eq!(parse_whisper_username("ChatRedux::AddTab: Adding tab with channel name: RRegion to index 1"), None);
    }

    #[test]
    fn trade_flow() {
        let mut p = LineParser::default();
        let mut ev = Vec::new();
        for line in [
            "100.0 Script [Info]: Dialog.lua: Dialog::CreateOkCancel(description=Are you sure you want to accept this trade? You are offering:",
            "Ash Prime Systems Blueprint",
            "Lex Prime Barrel x 2",
            "and will receive from Buyer\u{e000} the following:",
            "Platinum x 45, title= leftItem=/Menu/Confirm_Item_Ok",
            "101.0 Script [Info]: Trade.lua: The trade was successful!",
        ] {
            p.handle(line, &mut ev);
        }
        let trade = ev
            .iter()
            .find_map(|e| match e {
                LogEvent::TradeCompleted(t) => Some(t.clone()),
                _ => None,
            })
            .expect("trade event");
        assert_eq!(trade.partner, "Buyer");
        assert_eq!(trade.kind, TradeKind::Sale);
        assert_eq!(trade.platinum, 45);
        assert_eq!(trade.items.len(), 2);
        assert_eq!(trade.items[1].count, 2);
    }

    #[test]
    fn inventory_change_events() {
        let mut p = LineParser::default();
        let mut ev = Vec::new();
        for line in [
            "25185.383 Sys [Info]: OnInventoryResults completed in 71ms",
            "25200.508 Sys [Info]: Created /Lotus/Interface/PurchaseCelebration.swf",
            "25213.996 Script [Info]: TopMenu.lua: Abort: in hub (1)",
            "25216.617 Sys [Info]: SyncInventoryFromDB",
        ] {
            p.handle(line, &mut ev);
        }
        assert!(matches!(ev.as_slice(), [LogEvent::InventorySynced, LogEvent::PurchaseCompleted, LogEvent::MissionEnded]));
    }

    #[test]
    fn glyphs_and_framework() {
        assert_eq!(strip_platform_glyphs("Name\u{e000}"), "Name");
        assert!(is_log_framework_line("11828.904 Script [Info]: Dialog.lua: x"));
        assert!(!is_log_framework_line("Lex Prime Barrel"));
        assert_eq!(strip_dialog_arg_tail("Platinum x 45, title= leftItem=/x"), "Platinum x 45");
    }
}
