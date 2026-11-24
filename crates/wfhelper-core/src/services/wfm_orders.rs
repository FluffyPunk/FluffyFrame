//! Order books and the user's own listings (original: `services/wfmOrders.ts`).

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::wfm_client::{self, Api};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Sell,
    Buy,
}

impl OrderType {
    pub fn label(self) -> &'static str {
        match self {
            OrderType::Sell => "WTS",
            OrderType::Buy => "WTB",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub item_id: String,
    pub order_type: OrderType,
    pub platinum: u32,
    pub quantity: u32,
    pub rank: Option<u32>,
    pub visible: bool,
    pub user_name: Option<String>,
    /// warframe.market profile slug.
    pub user_slug: Option<String>,
    pub user_status: Option<String>,
    pub reputation: Option<i64>,
}

impl Order {
    pub fn profile_url(&self) -> Option<String> {
        self.user_slug.as_ref().map(|s| format!("https://warframe.market/profile/{s}"))
    }

    /// `/w <name> Hi! I want to buy: "<item>" for <plat> platinum. (warframe.market)`
    pub fn whisper(&self, item_name: &str) -> Option<String> {
        let name = self.user_name.as_deref()?;
        let verb = match self.order_type {
            OrderType::Sell => "buy",
            OrderType::Buy => "sell",
        };
        Some(format!(
            "/w {name} Hi! I want to {verb}: \"{item_name}\" for {} platinum. (warframe.market)",
            self.platinum
        ))
    }
}

fn parse_order(v: &Value) -> Option<Order> {
    let user = v.get("user");
    Some(Order {
        id: v.get("id")?.as_str()?.to_string(),
        item_id: v.get("itemId").and_then(Value::as_str).unwrap_or_default().to_string(),
        order_type: match v.get("type")?.as_str()? {
            "buy" => OrderType::Buy,
            _ => OrderType::Sell,
        },
        platinum: v.get("platinum").and_then(Value::as_u64).unwrap_or(0) as u32,
        quantity: v.get("quantity").and_then(Value::as_u64).unwrap_or(1) as u32,
        rank: v.get("rank").and_then(Value::as_u64).map(|r| r as u32),
        visible: v.get("visible").and_then(Value::as_bool).unwrap_or(true),
        user_name: user.and_then(|u| u.get("ingameName")).and_then(Value::as_str).map(str::to_string),
        user_slug: user.and_then(|u| u.get("slug")).and_then(Value::as_str).map(str::to_string),
        user_status: user.and_then(|u| u.get("status")).and_then(Value::as_str).map(str::to_string),
        reputation: user.and_then(|u| u.get("reputation")).and_then(Value::as_i64),
    })
}

fn parse_list(v: &Value) -> Vec<Order> {
    v.as_array().map(|a| a.iter().filter_map(parse_order).collect()).unwrap_or_default()
}

#[derive(Debug, Clone, Default)]
pub struct TopOrders {
    pub sell: Vec<Order>,
    pub buy: Vec<Order>,
}

/// Best online orders for an item (v2 `/orders/item/{slug}/top`).
pub fn top_orders(slug: &str, rank: Option<u32>) -> crate::Result<TopOrders> {
    let path = match rank {
        Some(r) => format!("/orders/item/{slug}/top?rank={r}"),
        None => format!("/orders/item/{slug}/top"),
    };
    let data = wfm_client::unwrap_v2(wfm_client::get(Api::V2, &path)?)?;
    Ok(TopOrders {
        sell: parse_list(data.get("sell").unwrap_or(&Value::Null)),
        buy: parse_list(data.get("buy").unwrap_or(&Value::Null)),
    })
}

pub fn my_orders() -> crate::Result<Vec<Order>> {
    let data = wfm_client::unwrap_v2(wfm_client::request(Api::V2, Method::GET, "/orders/my", None, true)?)?;
    Ok(parse_list(&data))
}

#[derive(Debug, Clone)]
pub struct NewOrder {
    pub item_id: String,
    pub order_type: OrderType,
    pub platinum: u32,
    pub quantity: u32,
    pub rank: Option<u32>,
    pub visible: bool,
}

pub fn create_order(order: &NewOrder) -> crate::Result<Order> {
    let mut body = json!({
        "itemId": order.item_id,
        "type": order.order_type,
        "platinum": order.platinum,
        "quantity": order.quantity.max(1),
        "visible": order.visible,
    });
    if let Some(rank) = order.rank {
        body["rank"] = json!(rank);
    }
    let data = wfm_client::unwrap_v2(wfm_client::request(Api::V2, Method::POST, "/order", Some(&body), true)?)?;
    parse_order(&data).ok_or_else(|| anyhow::anyhow!("unexpected order response"))
}

pub fn update_order(id: &str, platinum: u32, quantity: u32, visible: bool) -> crate::Result<()> {
    let body = json!({ "platinum": platinum, "quantity": quantity.max(1), "visible": visible });
    wfm_client::request(Api::V2, Method::PATCH, &format!("/order/{id}"), Some(&body), true)?;
    Ok(())
}

pub fn delete_order(id: &str) -> crate::Result<()> {
    wfm_client::request(Api::V2, Method::DELETE, &format!("/order/{id}"), None, true)?;
    Ok(())
}
