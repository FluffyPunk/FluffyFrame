//! warframe.market item catalog (original: `services/wfmCatalog.ts`).
//!
//! v2 `/items` carries each item's `gameRef` (DE unique name), which is the join
//! between the inventory and the market.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::json_cache;
use super::wfm_client::{self, Api};
use crate::config::constants::{WFM_ASSETS, WFM_CATALOG_TTL};

const CACHE: &str = "wfm/items-v2.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub game_ref: Option<String>,
    pub ducats: Option<u32>,
    pub max_rank: Option<u32>,
    pub tags: Vec<String>,
    pub thumb: Option<String>,
}

impl CatalogItem {
    pub fn thumb_url(&self) -> Option<String> {
        self.thumb.as_ref().map(|t| format!("{WFM_ASSETS}{t}"))
    }
    pub fn market_url(&self) -> String {
        format!("https://warframe.market/items/{}", self.slug)
    }
    pub fn is_set(&self) -> bool {
        self.slug.ends_with("_set")
    }
}

#[derive(Default)]
pub struct Catalog {
    pub items: Vec<CatalogItem>,
    by_game_ref: HashMap<String, usize>,
    by_slug: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
}

impl Catalog {
    pub fn from_items(items: Vec<CatalogItem>) -> Self {
        let mut c = Catalog { items, ..Default::default() };
        for (i, item) in c.items.iter().enumerate() {
            if let Some(r) = &item.game_ref {
                c.by_game_ref.insert(r.to_lowercase(), i);
            }
            c.by_slug.insert(item.slug.clone(), i);
            c.by_name.insert(item.name.to_lowercase(), i);
        }
        c
    }

    pub fn by_game_ref(&self, unique: &str) -> Option<&CatalogItem> {
        self.by_game_ref.get(&unique.to_lowercase()).map(|&i| &self.items[i])
    }
    pub fn by_slug(&self, slug: &str) -> Option<&CatalogItem> {
        self.by_slug.get(slug).map(|&i| &self.items[i])
    }
    pub fn by_name(&self, name: &str) -> Option<&CatalogItem> {
        self.by_name.get(&name.trim().to_lowercase()).map(|&i| &self.items[i])
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<&CatalogItem> {
        let q = query.trim().to_lowercase();
        if q.len() < 2 {
            return Vec::new();
        }
        let mut hits: Vec<&CatalogItem> =
            self.items.iter().filter(|i| i.name.to_lowercase().contains(&q)).collect();
        hits.sort_by_key(|i| (!i.name.to_lowercase().starts_with(&q), i.name.len()));
        hits.truncate(limit);
        hits
    }
}

fn normalise(v: &Value) -> Option<CatalogItem> {
    let en = v.pointer("/i18n/en");
    Some(CatalogItem {
        id: v.get("id")?.as_str()?.to_string(),
        slug: v.get("slug")?.as_str()?.to_string(),
        name: en.and_then(|e| e.get("name")).and_then(Value::as_str)?.to_string(),
        game_ref: v.get("gameRef").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
        ducats: v.get("ducats").and_then(Value::as_u64).map(|n| n as u32),
        max_rank: v.get("maxRank").and_then(Value::as_u64).map(|n| n as u32),
        tags: v
            .get("tags")
            .and_then(Value::as_array)
            .map(|t| t.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default(),
        thumb: en.and_then(|e| e.get("thumb")).and_then(Value::as_str).map(str::to_string),
    })
}

pub fn parse(text: &str) -> crate::Result<Catalog> {
    let value: Value = serde_json::from_str(text)?;
    let data = wfm_client::unwrap_v2(value)?;
    let items: Vec<CatalogItem> = data
        .as_array()
        .map(|a| a.iter().filter_map(normalise).collect())
        .unwrap_or_default();
    anyhow::ensure!(!items.is_empty(), "warframe.market catalog is empty");
    Ok(Catalog::from_items(items))
}

pub fn load() -> crate::Result<Catalog> {
    let text = json_cache::fetch_cached(CACHE, WFM_CATALOG_TTL, || {
        Ok(wfm_client::get(Api::V2, "/items")?.to_string())
    })?;
    parse(&text)
}
