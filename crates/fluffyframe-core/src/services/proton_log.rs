//! Linux: the game's log lines as it writes them, from Proton's log. Warframe
//! sends every EE.log line through `OutputDebugStringA` too, and with
//! `PROTON_LOG=1` Wine writes each call to `~/steam-230410*.log` at once,
//! while EE.log reaches the disk in batches ~10 s late:
//!
//! `48619.503:0554:0560:warn:debugstr:OutputDebugStringA "121.897 Script [Info]: …\n"`
//!
//! Wine escapes the text (`\n`, `\"`, `\xee` for bytes past ASCII) and cuts
//! strings past ~300 characters (`"...`): those are skipped, EE.log has them
//! whole.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

const MARKER: &str = ":debugstr:OutputDebugStringA \"";
/// Bytes read per call at most (a busy game logs a lot).
const MAX_READ: u64 = 4 * 1024 * 1024;

/// Following the newest Proton log of Warframe.
#[derive(Default)]
pub struct ProtonLog {
    path: Option<PathBuf>,
    offset: u64,
    remainder: Vec<u8>,
}

impl ProtonLog {
    /// Picks the newest `steam-230410*.log` in the home folder (Proton's
    /// default place; some builds add their version to the name). A new file
    /// is followed from its end: what's in it already was handled or missed.
    pub fn resolve(&mut self) {
        let found = std::env::var_os("HOME").and_then(|home| newest_log(&PathBuf::from(home)));
        if found != self.path {
            if let Some(p) = &found {
                log::info!("following the Proton log for live game events: {}", p.display());
            }
            self.offset = found.as_ref().and_then(|p| std::fs::metadata(p).ok()).map_or(0, |m| m.len());
            self.remainder.clear();
            self.path = found;
        }
    }

    pub fn found(&self) -> bool {
        self.path.is_some()
    }

    /// The game's log lines written since the last call, whole ones only.
    pub fn read(&mut self) -> Vec<String> {
        let Some(path) = &self.path else { return Vec::new() };
        let Ok(mut file) = std::fs::File::open(path) else { return Vec::new() };
        let len = file.metadata().map_or(0, |m| m.len());
        if len < self.offset {
            // A new game session overwrote it: all of it is new.
            self.offset = 0;
            self.remainder.clear();
        }
        if len == self.offset {
            return Vec::new();
        }
        let mut buf = Vec::new();
        if file.seek(SeekFrom::Start(self.offset)).is_err() || file.take((len - self.offset).min(MAX_READ)).read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        self.offset += buf.len() as u64;
        self.remainder.extend_from_slice(&buf);
        let Some(end) = self.remainder.iter().rposition(|&b| b == b'\n') else { return Vec::new() };
        let complete: Vec<u8> = self.remainder.drain(..=end).collect();
        String::from_utf8_lossy(&complete).lines().filter_map(debug_string).collect()
    }
}

fn newest_log(home: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_dir(home)
        .ok()?
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.starts_with("steam-230410") && name.ends_with(".log")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

/// The text a Proton log line carries for the game's `OutputDebugStringA`,
/// unescaped; `None` for other lines and for strings Wine cut short.
pub fn debug_string(line: &str) -> Option<String> {
    let start = line.find(MARKER)? + MARKER.len();
    let quoted = line[start..].trim_end().strip_suffix('"')?;
    let mut bytes = Vec::with_capacity(quoted.len());
    let mut chars = quoted.bytes();
    while let Some(b) = chars.next() {
        if b != b'\\' {
            bytes.push(b);
            continue;
        }
        match chars.next()? {
            b'n' => bytes.push(b'\n'),
            b'r' => bytes.push(b'\r'),
            b't' => bytes.push(b'\t'),
            b'x' => {
                let hex = [chars.next()?, chars.next()?];
                bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
            }
            other => bytes.push(other),
        }
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_the_games_text() {
        let line = r#"48619.503:0554:0560:warn:debugstr:OutputDebugStringA "121.897 Script [Info]: Dialog.lua: Dialog::SendResult(4)\n""#;
        assert_eq!(debug_string(line).as_deref(), Some("121.897 Script [Info]: Dialog.lua: Dialog::SendResult(4)\n"));
        // Bytes past ASCII come back as UTF-8 (the name's platform glyph), quotes and tabs too.
        let line = r#"1.0:0554:0560:warn:debugstr:OutputDebugStringA "Logged in 0xFFP\xee\x80\x80 \"x\"\tok\n""#;
        assert_eq!(debug_string(line).as_deref(), Some("Logged in 0xFFP\u{e000} \"x\"\tok\n"));
    }

    #[test]
    fn skips_cut_strings_and_other_channels() {
        assert_eq!(debug_string(r#"1.0:0554:0560:warn:debugstr:OutputDebugStringA "16.692 Script [Info]: wepTypes=/Lotus/Weapo"..."#), None);
        assert_eq!(debug_string(r#"1.0:0554:0560:warn:seh:OutputDebugStringW L"2.837 Script [Info]: x\n""#), None);
        assert_eq!(debug_string("1.0:0554:0560:fixme:ntdll:NtQuerySystemInformation"), None);
    }

    #[test]
    fn follows_new_lines_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("steam-230410.log");
        std::fs::write(&path, "old:warn:debugstr:OutputDebugStringA \"1.0 Sys [Info]: before\\n\"\n").unwrap();
        let mut log =
            ProtonLog { path: newest_log(dir.path()), offset: std::fs::metadata(&path).unwrap().len(), ..Default::default() };
        assert!(log.read().is_empty(), "what was there is history");
        let mut file = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        std::io::Write::write_all(&mut file, b"2:warn:debugstr:OutputDebugStringA \"2.0 Sys [Info]: now\\n\"\n3:warn:debugstr:OutputDebugStringA \"3.0 Sys").unwrap();
        // The half-written line waits for its end.
        assert_eq!(log.read(), ["2.0 Sys [Info]: now\n"]);
        std::io::Write::write_all(&mut file, b" [Info]: later\\n\"\n").unwrap();
        assert_eq!(log.read(), ["3.0 Sys [Info]: later\n"]);
    }
}
