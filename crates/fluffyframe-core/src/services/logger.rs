//! Logging (original: `services/logger.ts`): stderr plus a size-capped file in
//! the data directory. Account identifiers never reach the log.

use std::io::Write;
use std::sync::Mutex;

use log::{Level, LevelFilter, Metadata, Record};

use crate::config::paths;

const MAX_LOG_BYTES: u64 = 4 * 1024 * 1024;

struct Logger {
    file: Mutex<Option<std::fs::File>>,
    level: LevelFilter,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
            && ((metadata.target().starts_with("fluffyframe") || metadata.target().starts_with("wf_api")) || metadata.level() <= Level::Warn)
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} {:<5} [{}] {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.target(),
            redact(&record.args().to_string())
        );
        eprint!("{line}");
        if let Ok(mut guard) = self.file.lock()
            && let Some(file) = guard.as_mut()
        {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock()
            && let Some(file) = guard.as_mut()
        {
            let _ = file.flush();
        }
    }
}

/// Masks `accountId=` and `nonce=` values (original: `services/logPrivacy.ts`).
pub fn redact(text: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(accountId=|nonce=)[0-9a-fA-F]+").unwrap());
    re.replace_all(text, "${1}<redacted>").into_owned()
}

pub fn init() {
    let path = paths::log_file();
    if std::fs::metadata(&path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).open(&path).ok();
    let level = if crate::config::app_meta::env_var("DEBUG").is_some() {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let logger = Logger { file: Mutex::new(file), level };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(level);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn redacts_auth() {
        assert_eq!(
            super::redact("?accountId=abc123&nonce=999"),
            "?accountId=<redacted>&nonce=<redacted>"
        );
    }
}
