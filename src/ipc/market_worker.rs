//! warframe.market account and lookups (original: `ipc/wfmIpc.ts`, `ipc/rivensIpc.ts`).

use std::sync::Arc;

use crossbeam_channel::Receiver;
use fluffyframe_core::services::wfm_orders::{self, NewOrder};
use fluffyframe_core::services::wfm_stats_price::Variant;
use fluffyframe_core::services::{wfm_client, wfm_riven_search};

use super::{Emitter, Event, ToastKind};

pub enum MarketCmd {
    TopOrders { slug: String, variant: Variant },
    SignIn { email: String, password: String },
    SignOut,
    MyOrders,
    CreateOrders(Vec<NewOrder>),
    UpdateOrder { id: String, platinum: u32, quantity: u32, visible: bool },
    DeleteOrder(String),
    RivenWeapons,
    RivenAuctions(String),
    /// List one of the user's rivens (direct sale or auction).
    CreateRivenAuction(Box<wfm_riven_search::RivenListing>),
    /// The user's open riven listings.
    MyAuctions,
    UpdateAuction { auction: Box<wfm_riven_search::RivenAuction>, price: u32, visible: bool },
    CloseAuction(String),
}

pub fn top_orders_key(slug: &str, variant: Variant) -> String {
    fluffyframe_core::services::wfm_stats_price::price_key(slug, variant)
}

pub fn run(emitter: Emitter, rx: Receiver<MarketCmd>) {
    emitter.emit(Event::Session(wfm_client::session()));
    while let Ok(cmd) = rx.recv() {
        match cmd {
            MarketCmd::TopOrders { slug, variant } => match wfm_orders::top_orders(&slug, variant) {
                Ok(orders) => emitter.emit(Event::TopOrders { key: top_orders_key(&slug, variant), orders }),
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
                    refresh_auctions(&emitter);
                }
                Err(e) => emitter.toast(ToastKind::Error, e.to_string()),
            },
            MarketCmd::SignOut => {
                wfm_client::sign_out();
                emitter.emit(Event::Session(None));
                emitter.emit(Event::MyOrders(Vec::new()));
                emitter.emit(Event::MyAuctions(Vec::new()));
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
            MarketCmd::CreateRivenAuction(listing) => match wfm_riven_search::create_auction(&listing) {
                Ok(_) => {
                    let what = if matches!(listing.kind, wfm_riven_search::ListingKind::Direct { .. }) { "for sale" } else { "for auction" };
                    emitter.toast(ToastKind::Success, format!("Riven listed {what} on warframe.market"));
                    // The similar-listings panel then shows it too.
                    if let Ok(auctions) = wfm_riven_search::search_auctions(&listing.weapon_slug) {
                        emitter.emit(Event::RivenAuctions { slug: listing.weapon_slug.clone(), auctions });
                    }
                    refresh_auctions(&emitter);
                }
                Err(e) => emitter.toast(ToastKind::Error, format!("Listing failed: {e}")),
            },
            MarketCmd::MyAuctions => refresh_auctions(&emitter),
            MarketCmd::UpdateAuction { auction, price, visible } => {
                match wfm_riven_search::update_auction(&auction, price, visible) {
                    Ok(()) => emitter.toast(ToastKind::Success, "Contract updated"),
                    Err(e) => emitter.toast(ToastKind::Error, format!("Update failed: {e}")),
                }
                refresh_auctions(&emitter);
            }
            MarketCmd::CloseAuction(id) => {
                match wfm_riven_search::close_auction(&id) {
                    Ok(()) => emitter.toast(ToastKind::Success, "Contract removed"),
                    Err(e) => emitter.toast(ToastKind::Error, format!("Remove failed: {e}")),
                }
                refresh_auctions(&emitter);
            }
        }
    }
}

/// The user's open riven listings, by their warframe.market profile.
fn refresh_auctions(emitter: &Emitter) {
    let Some(profile) = wfm_client::session().and_then(|s| s.slug.or(s.ingame_name)) else { return };
    match wfm_riven_search::my_auctions(&profile) {
        Ok(list) => emitter.emit(Event::MyAuctions(list)),
        Err(e) => emitter.toast(ToastKind::Error, format!("Could not load your riven contracts: {e}")),
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
