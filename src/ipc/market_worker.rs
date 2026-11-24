//! warframe.market account and lookups (original: `ipc/wfmIpc.ts`, `ipc/rivensIpc.ts`).

use std::sync::Arc;

use crossbeam_channel::Receiver;
use wfhelper_core::services::wfm_orders::{self, NewOrder};
use wfhelper_core::services::{wfm_client, wfm_riven_search};

use super::{Emitter, Event, ToastKind};

pub enum MarketCmd {
    TopOrders { slug: String, rank: Option<u32> },
    SignIn { email: String, password: String },
    SignOut,
    MyOrders,
    CreateOrders(Vec<NewOrder>),
    UpdateOrder { id: String, platinum: u32, quantity: u32, visible: bool },
    DeleteOrder(String),
    RivenWeapons,
    RivenAuctions(String),
}

pub fn top_orders_key(slug: &str, rank: Option<u32>) -> String {
    wfhelper_core::services::wfm_stats_price::price_key(slug, rank)
}

pub fn run(emitter: Emitter, rx: Receiver<MarketCmd>) {
    emitter.emit(Event::Session(wfm_client::session()));
    while let Ok(cmd) = rx.recv() {
        match cmd {
            MarketCmd::TopOrders { slug, rank } => match wfm_orders::top_orders(&slug, rank) {
                Ok(orders) => emitter.emit(Event::TopOrders { key: top_orders_key(&slug, rank), orders }),
                Err(e) => emitter.toast(ToastKind::Error, format!("Order book for {slug}: {e}")),
            },
            MarketCmd::SignIn { email, password } => match wfm_client::sign_in(&email, &password) {
                Ok(session) => {
                    emitter.toast(
                        ToastKind::Success,
                        format!("Signed in as {}", session.ingame_name.as_deref().unwrap_or("warframe.market user")),
                    );
                    emitter.emit(Event::Session(Some(session)));
                    refresh_orders(&emitter);
                }
                Err(e) => emitter.toast(ToastKind::Error, e.to_string()),
            },
            MarketCmd::SignOut => {
                wfm_client::sign_out();
                emitter.emit(Event::Session(None));
                emitter.emit(Event::MyOrders(Vec::new()));
            }
            MarketCmd::MyOrders => refresh_orders(&emitter),
            MarketCmd::CreateOrders(orders) => {
                let total = orders.len();
                let mut ok = 0;
                for order in orders {
                    match wfm_orders::create_order(&order) {
                        Ok(_) => ok += 1,
                        Err(e) => emitter.toast(ToastKind::Error, format!("Listing failed: {e}")),
                    }
                }
                if ok > 0 {
                    emitter.toast(ToastKind::Success, format!("Listed {ok} of {total} orders"));
                }
                refresh_orders(&emitter);
            }
            MarketCmd::UpdateOrder { id, platinum, quantity, visible } => {
                match wfm_orders::update_order(&id, platinum, quantity, visible) {
                    Ok(()) => emitter.toast(ToastKind::Success, "Order updated"),
                    Err(e) => emitter.toast(ToastKind::Error, format!("Update failed: {e}")),
                }
                refresh_orders(&emitter);
            }
            MarketCmd::DeleteOrder(id) => {
                match wfm_orders::delete_order(&id) {
                    Ok(()) => emitter.toast(ToastKind::Success, "Order removed"),
                    Err(e) => emitter.toast(ToastKind::Error, format!("Remove failed: {e}")),
                }
                refresh_orders(&emitter);
            }
            MarketCmd::RivenWeapons => match wfm_riven_search::load_weapons() {
                Ok(w) => emitter.emit(Event::RivenWeapons(Arc::new(w))),
                Err(e) => emitter.toast(ToastKind::Error, format!("Riven weapons unavailable: {e}")),
            },
            MarketCmd::RivenAuctions(slug) => match wfm_riven_search::search_auctions(&slug) {
                Ok(auctions) => emitter.emit(Event::RivenAuctions { slug, auctions }),
                Err(e) => emitter.toast(ToastKind::Error, format!("Riven search failed: {e}")),
            },
        }
    }
}

fn refresh_orders(emitter: &Emitter) {
    if wfm_client::session().is_none() {
        return;
    }
    match wfm_orders::my_orders() {
        Ok(orders) => emitter.emit(Event::MyOrders(orders)),
        Err(e) => {
            emitter.toast(ToastKind::Error, format!("Could not load your orders: {e}"));
            emitter.emit(Event::Session(wfm_client::session()));
        }
    }
}
