//! Bounty details the world state doesn't spell out.
//!
//! Zariman, Cavia and Hex bounties appear in the world state only as a seed the
//! game expands itself; browse.wf's oracle serves them expanded (node, challenge,
//! Hex ally). The community export's English text turns challenges into a task
//! and gives open-world jobs their in-game names.

use std::collections::HashMap;

use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use super::{http, json_cache};
use crate::config::constants::{BOUNTY_CYCLE_URL, PEP_BASE, PEP_TTL};
use crate::config::game_tables::{faction_label, split_camel, title_case};

/// One of the seeded bounties (Zariman, Cavia, Hex).
#[derive(Debug, Clone, PartialEq)]
pub struct SeededBounty {
    pub node: String,
    /// Challenge name: "Efficient Extermination".
    pub title: String,
    /// What to do: "Complete mission in under 6 minutes".
    pub task: String,
    /// Hex protoframe that joins the mission: "Lettie".
    pub ally: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BountyBook {
    /// When the seeded bounties rotate.
    pub expiry: Option<DateTime<Utc>>,
    /// Who the Zariman bounties fight this rotation ("Grineer").
    pub zariman_faction: Option<String>,
    /// Syndicate tag → its bounties, in board order.
    pub seeded: HashMap<String, Vec<SeededBounty>>,
    /// Open-world job type → in-game name ("Cull The Enemy").
    pub job_names: HashMap<String, String>,
}

/// English text for the language keys the exports point at.
struct Text {
    dict: HashMap<String, String>,
    challenges: Value,
}

impl Text {
    fn get(&self, key: &str) -> Option<&str> {
        self.dict.get(key).map(String::as_str).filter(|t| !t.is_empty())
    }

    /// (name, task) for a challenge; the task is the description's last line
    /// with its `|COUNT|` filled in, as the game shows it.
    fn challenge(&self, path: &str) -> (String, String) {
        let c = &self.challenges[path];
        let key = |k: &str| c.get(k).and_then(Value::as_str).and_then(|k| self.get(k));
        let fallback = || split_camel(path.rsplit('/').next().unwrap_or(path).trim_end_matches("Challenge"));
        let count = c.get("requiredCount").and_then(Value::as_u64).unwrap_or(1);
        let task = key("description").and_then(|d| d.lines().last()).map(|d| d.replace("|COUNT|", &count.to_string()));
        (key("name").map_or_else(fallback, str::to_string), task.unwrap_or_default())
    }
}

/// "/…/ProtoframeAllies/LettieAllyAgent" → "Lettie".
fn ally_name(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    split_camel(name.strip_suffix("AllyAgent").unwrap_or(name))
}

fn parse_cycle(root: &Value, text: &Text, book: &mut BountyBook) {
    book.expiry = root.get("expiry").and_then(Value::as_i64).and_then(|ms| Utc.timestamp_millis_opt(ms).single());
    book.zariman_faction = root.get("zarimanFaction").and_then(Value::as_str).map(faction_label);
    for (tag, list) in root.get("bounties").and_then(Value::as_object).into_iter().flatten() {
        let bounties = list
            .as_array()
            .into_iter()
            .flatten()
            .map(|b| {
                let s = |k: &str| b.get(k).and_then(Value::as_str);
                let (title, task) = text.challenge(s("challenge").unwrap_or(""));
                SeededBounty { node: s("node").unwrap_or("").to_string(), title, task, ally: s("ally").map(ally_name) }
            })
            .collect();
        book.seeded.insert(tag.clone(), bounties);
    }
}

fn parse_job_names(bounties: &Value, text: &Text) -> HashMap<String, String> {
    bounties
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(job, b)| Some((job.clone(), title_case(text.get(b.get("name")?.as_str()?)?))))
        .collect()
}

fn pep(file: &str) -> crate::Result<Value> {
    let text = json_cache::fetch_cached(&format!("pep/{file}"), PEP_TTL, || http::get_text(&format!("{PEP_BASE}{file}")))?;
    Ok(serde_json::from_str(&text)?)
}

/// Fetches the current bounty rotation (never cached: it changes every few
/// hours) and the export text (cached for a week).
pub fn load() -> crate::Result<BountyBook> {
    let text = Text { dict: serde_json::from_value(pep("dict.en.json")?)?, challenges: pep("ExportChallenges.json")? };
    let mut book = BountyBook { job_names: parse_job_names(&pep("ExportBounties.json")?, &text), ..Default::default() };
    parse_cycle(&serde_json::from_str(&http::get_text(BOUNTY_CYCLE_URL)?)?, &text, &mut book);
    Ok(book)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text() -> Text {
        let dict = [
            ("/Ch/FastName", "Efficient Extermination"),
            ("/Ch/FastDesc", "Bonus objective\r\nComplete mission in under |COUNT| minutes"),
            ("/Jobs/CullName", "CULL THE ENEMY"),
        ];
        Text {
            dict: dict.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            challenges: serde_json::json!({ "/C/Fast": { "name": "/Ch/FastName", "description": "/Ch/FastDesc", "requiredCount": 6 } }),
        }
    }

    #[test]
    fn expands_seeded_bounties() {
        let root = serde_json::json!({ "expiry": 1795745165513i64, "zarimanFaction": "FC_GRINEER", "bounties": {
            "ZarimanSyndicate": [{ "node": "SolNode231", "challenge": "/C/Fast" }],
            "HexSyndicate": [{ "node": "SolNode858", "challenge": "/C/KillFromAboveChallenge", "ally": "/A/LettieAllyAgent" }],
        } });
        let mut book = BountyBook::default();
        parse_cycle(&root, &text(), &mut book);
        assert_eq!(book.zariman_faction.as_deref(), Some("Grineer"));
        let z = &book.seeded["ZarimanSyndicate"][0];
        assert_eq!((z.title.as_str(), z.task.as_str()), ("Efficient Extermination", "Complete mission in under 6 minutes"));
        // A challenge the export lacks still gets a readable name.
        let h = &book.seeded["HexSyndicate"][0];
        assert_eq!((h.title.as_str(), h.ally.as_deref()), ("Kill From Above", Some("Lettie")));
    }

    #[test]
    fn names_open_world_jobs() {
        let bounties = serde_json::json!({ "/Jobs/Cull": { "name": "/Jobs/CullName" }, "/Jobs/Unnamed": { "name": "/Jobs/Missing" } });
        let names = parse_job_names(&bounties, &text());
        assert_eq!(names.get("/Jobs/Cull").map(String::as_str), Some("Cull The Enemy"));
        assert!(!names.contains_key("/Jobs/Unnamed"));
    }
}
