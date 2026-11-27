//! Interface translations.
//!
//! English is the source language: the text in the code is the key, and
//! `assets/locales/<code>.json` maps it to a translation. Anything a locale lacks
//! shows in English.
//!
//! - [`tr`]: plain text.
//! - [`trf`]: text with values: `{}` in order, or `{0}`, `{1}`… when a
//!   translation needs another order.
//! - [`trn`]: counted text. A translation is a string, or a list of plural forms
//!   in the order of that language's [`Locale::plural`] rule.
//! - [`tkey`]: marks text kept as data (a filter value, a table entry) that is
//!   shown elsewhere with `tr(value)`, so it still gets translated.
//!
//! A locale file groups its strings into sections by where they show ("Mastery",
//! "Settings", "Game: mission types"…); "Common" holds those used in several
//! places. Sections only help finding an entry: each string appears in exactly
//! one, and the lookup ignores them.
//!
//! Adding a language: copy `uk.json` to `<code>.json`, translate its values, and
//! add a [`Locale`] line to [`LOCALES`]. Adding a string: put it in the section
//! of the view that shows it. `cargo test` checks that every locale file covers
//! every string in the code, once.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Locale {
    /// Settings value ("uk").
    pub code: &'static str,
    /// Its own name for itself ("Українська").
    pub name: &'static str,
    /// The translation file; None for English, the source.
    file: Option<&'static str>,
    /// Plural form index for a count.
    plural: fn(u64) -> usize,
}

pub const LOCALES: &[Locale] = &[
    Locale { code: "en", name: "English", file: None, plural: plural_one_other },
    Locale { code: "uk", name: "Українська", file: Some(include_str!("../assets/locales/uk.json")), plural: plural_east_slavic },
];

/// English and many others: one, other.
fn plural_one_other(n: u64) -> usize {
    usize::from(n != 1)
}

/// Ukrainian (and Russian, Belarusian): one (1, 21…), few (2–4, 22–24…), many.
fn plural_east_slavic(n: u64) -> usize {
    match (n % 10, n % 100) {
        (1, r) if r != 11 => 0,
        (2..=4, r) if !(12..=14).contains(&r) => 1,
        _ => 2,
    }
}

#[derive(Debug)]
enum Entry {
    Text(String),
    Forms(Vec<String>),
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static TABLES: OnceLock<Vec<HashMap<String, Entry>>> = OnceLock::new();

/// Every (key, entry) of a locale file, through its sections.
fn entries(map: serde_json::Map<String, serde_json::Value>, out: &mut Vec<(String, Entry)>) {
    for (k, v) in map {
        match v {
            serde_json::Value::Object(section) => entries(section, out),
            serde_json::Value::String(s) => out.push((k, Entry::Text(s))),
            serde_json::Value::Array(a) => {
                out.push((k, Entry::Forms(a.into_iter().filter_map(|f| f.as_str().map(str::to_string)).collect())));
            }
            _ => {}
        }
    }
}

fn parse(file: &str) -> HashMap<String, Entry> {
    let raw: serde_json::Map<String, serde_json::Value> = serde_json::from_str(file).unwrap_or_else(|e| {
        log::error!("locale file: {e}");
        Default::default()
    });
    let mut out = Vec::new();
    entries(raw, &mut out);
    out.into_iter().collect()
}

fn tables() -> &'static [HashMap<String, Entry>] {
    TABLES.get_or_init(|| LOCALES.iter().map(|l| l.file.map(parse).unwrap_or_default()).collect())
}

/// Switches the interface language; unknown codes fall back to English.
pub fn set_locale(code: &str) {
    CURRENT.store(LOCALES.iter().position(|l| l.code == code).unwrap_or(0), Ordering::Relaxed);
}

fn entry(key: &str) -> Option<(&'static Entry, &'static Locale)> {
    let i = CURRENT.load(Ordering::Relaxed);
    tables()[i].get(key).map(|e| (e, &LOCALES[i]))
}

/// Marks `text` for translation without translating it (gettext's `N_`): for
/// strings that are also data, translated where shown with `tr(value)`.
pub const fn tkey(text: &'static str) -> &'static str {
    text
}

/// `text` in the current language.
pub fn tr(text: &'static str) -> &'static str {
    match entry(text) {
        Some((Entry::Text(t), _)) => t,
        Some((Entry::Forms(f), _)) => f.first().map_or(text, String::as_str),
        None => text,
    }
}

/// Text only known at run time (a mission type, a planet) in the current
/// language, when a locale has it; its keys are listed with [`tkey`] (see
/// `core_text`).
pub fn tr_any(text: &str) -> &str {
    match entry(text) {
        Some((Entry::Text(t), _)) => t,
        _ => text,
    }
}

/// `template` in the current language, with `args` filled in.
pub fn trf(template: &'static str, args: &[&dyn Display]) -> String {
    fill(tr(template), args)
}

/// Counted text: English `one` or `other` by `n`, or the current language's
/// form for `n` (keyed by `other`), with `args` filled in.
pub fn trn(one: &'static str, other: &'static str, n: u64, args: &[&dyn Display]) -> String {
    let template = match entry(other) {
        Some((Entry::Forms(forms), locale)) => {
            let i = (locale.plural)(n).min(forms.len().saturating_sub(1));
            forms.get(i).map_or(other, String::as_str)
        }
        Some((Entry::Text(t), _)) => t,
        None if n == 1 => one,
        None => other,
    };
    fill(template, args)
}

/// Replaces `{}` with the next argument and `{N}` with argument N.
fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if after[..end].chars().all(|c| c.is_ascii_digit()) => {
                let index = if end == 0 {
                    next += 1;
                    next - 1
                } else {
                    after[..end].parse().unwrap_or(usize::MAX)
                };
                match args.get(index) {
                    Some(a) => out.push_str(&a.to_string()),
                    None => out.push_str(&rest[start..start + end + 2]),
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_in_order_or_by_index() {
        assert_eq!(fill("{} of {}", &[&3, &5]), "3 of 5");
        assert_eq!(fill("{1}: {0}", &[&"a", &"b"]), "b: a");
        assert_eq!(fill("{x} {}", &[&1]), "{x} 1");
    }

    #[test]
    fn ukrainian_plural_forms() {
        let forms: Vec<usize> = [1, 2, 4, 5, 11, 12, 21, 22, 25, 111, 101].iter().map(|n| plural_east_slavic(*n)).collect();
        assert_eq!(forms, [0, 1, 1, 2, 2, 2, 0, 1, 2, 2, 0]);
    }

    /// Every `tr`/`trf`/`trn` string in the code, unescaped.
    fn keys_in_code() -> std::collections::BTreeSet<String> {
        fn literal(s: &str) -> Option<(String, &str)> {
            let s = s.trim_start().strip_prefix('"')?;
            let mut out = String::new();
            let mut chars = s.char_indices();
            while let Some((i, c)) = chars.next() {
                match c {
                    '"' => return Some((out, &s[i + 1..])),
                    '\\' => match chars.next()?.1 {
                        'n' => out.push('\n'),
                        't' => out.push('\t'),
                        other => out.push(other),
                    },
                    c => out.push(c),
                }
            }
            None
        }
        fn walk(dir: &std::path::Path, keys: &mut std::collections::BTreeSet<String>) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, keys);
                } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("i18n.rs") {
                    let src = std::fs::read_to_string(&path).unwrap();
                    for (call, literals) in [("tr(", 1), ("trf(", 1), ("trn(", 2), ("tkey(", 1)] {
                        for (at, _) in src.match_indices(call) {
                            // A call, not the end of another name (`attr(`).
                            if src[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
                                continue;
                            }
                            // `trn` is keyed by its second (plural) string.
                            let mut rest = &src[at + call.len()..];
                            for i in 0..literals {
                                let Some((key, after)) = literal(rest) else { break };
                                if i + 1 == literals {
                                    keys.insert(key);
                                }
                                rest = after.trim_start().strip_prefix(',').unwrap_or(after);
                            }
                        }
                    }
                }
            }
        }
        let mut keys = std::collections::BTreeSet::new();
        walk(std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src")), &mut keys);
        keys
    }

    #[test]
    fn locale_files_match_the_code() {
        let keys = keys_in_code();
        assert!(keys.len() > 100, "found only {} strings", keys.len());
        for (locale, table) in LOCALES.iter().zip(tables()) {
            if locale.file.is_none() {
                continue;
            }
            let mut all = Vec::new();
            entries(serde_json::from_str(locale.file.unwrap()).unwrap(), &mut all);
            let mut seen = std::collections::HashSet::new();
            let twice: Vec<&String> = all.iter().map(|(k, _)| k).filter(|k| !seen.insert(*k)).collect();
            assert!(twice.is_empty(), "{}: strings in more than one section: {twice:#?}", locale.code);
            let missing: Vec<&String> = keys.iter().filter(|k| !table.contains_key(*k)).collect();
            let unused: Vec<&String> = table.keys().filter(|k| !keys.contains(*k)).collect();
            assert!(missing.is_empty() && unused.is_empty(), "{}: missing {missing:#?}\nunused {unused:#?}", locale.code);
        }
    }
}
