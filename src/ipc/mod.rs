//! Background workers and the message types between them and the UI.
//!
//! Replaces the original's Electron main process + `ipc/*` handlers: the UI sends
//! commands over channels, workers reply with [`Event`]s and wake the UI.

mod data_worker;
mod inventory_worker;
mod log_router;
mod market_worker;
mod presence_worker;
mod price_worker;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, RwLock};

use crossbeam_channel::{Receiver, Sender, unbounded};
use fluffyframe_core::config::settings::Settings;
use fluffyframe_core::services::api_helper_runner::HelperStatus;
use fluffyframe_core::services::arbi_schedule::ArbiSchedule;
use fluffyframe_core::services::drop_data::DropIndex;
use fluffyframe_core::services::ee_log_monitor::LogEvent;
use fluffyframe_core::services::inventory::Inventory;
use fluffyframe_core::services::item_database::ItemDb;
use fluffyframe_core::services::trade_tracker::TradeEvent;
use fluffyframe_core::services::wfm_catalog::Catalog;
use fluffyframe_core::services::wfm_client::Session;
use fluffyframe_core::services::wfm_orders::{NewOrder, Order, TopOrders};
use fluffyframe_core::services::wfm_riven_search::{RivenAuction, RivenWeapons};
use fluffyframe_core::services::wfm_stats_price::{Price, PriceCache};
use fluffyframe_core::services::world_state::WorldState;

pub use data_worker::DataCmd;
pub use inventory_worker::InventoryCmd;
pub use log_router::LogCmd;
pub use market_worker::MarketCmd;
pub use presence_worker::PresenceCmd;
pub use price_worker::PriceCmd;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub enum Event {
    Loading(Option<String>),
    GameData(Arc<ItemDb>),
    Catalog(Arc<Catalog>),
    Inventory(Arc<Inventory>),
    Helper(HelperStatus),
    World(Arc<WorldState>),
    Arbi(Arc<ArbiSchedule>),
    Drops(Arc<DropIndex>),
    /// The user's warframe.market status (None: signed out or unknown).
    WfmStatus(Option<fluffyframe_core::services::wfm_presence::WfmStatus>),
    /// The user's open riven listings.
    MyAuctions(Vec<fluffyframe_core::services::wfm_riven_search::RivenAuction>),
    /// Empty when it couldn't be fetched.
    Bounties(Arc<fluffyframe_core::services::bounty_cycle::BountyBook>),
    /// Community riven good-rolls sheet (attribute grades).
    GoodRolls(Arc<fluffyframe_core::services::riven_good_rolls::GoodRolls>),
    Prices(Vec<(String, Price)>),
    PriceQueue(usize),
    TopOrders { key: String, orders: TopOrders },
    MyOrders(Vec<Order>),
    Session(Option<Session>),
    RivenWeapons(Arc<RivenWeapons>),
    RivenAuctions { slug: String, auctions: Vec<RivenAuction> },
    Log(LogEvent),
    Trade(TradeEvent),
    Toast(ToastKind, String),
}

/// State the workers share with the UI.
pub struct Shared {
    pub settings: RwLock<Settings>,
    pub db: RwLock<Option<Arc<ItemDb>>>,
    pub prices: PriceCache,
    /// EE.log saw a login in this game session.
    pub logged_in: AtomicBool,
}

/// Sends events and wakes the UI thread.
#[derive(Clone)]
pub struct Emitter {
    tx: Sender<Event>,
    ctx: egui::Context,
}

impl Emitter {
    pub fn emit(&self, event: Event) {
        if self.tx.send(event).is_ok() {
            self.ctx.request_repaint();
        }
    }
    pub fn toast(&self, kind: ToastKind, text: impl Into<String>) {
        self.emit(Event::Toast(kind, text.into()));
    }
}

pub struct Backend {
    pub shared: Arc<Shared>,
    pub events: Receiver<Event>,
    data: Sender<DataCmd>,
    inventory: Sender<InventoryCmd>,
    prices: Sender<PriceCmd>,
    market: Sender<MarketCmd>,
    log: Sender<LogCmd>,
    presence: Sender<PresenceCmd>,
    pub hotkey: crate::hotkey::Hotkey,
}

impl Backend {
    pub fn start(ctx: egui::Context, settings: Settings) -> Self {
        let shared = Arc::new(Shared {
            settings: RwLock::new(settings),
            db: RwLock::new(None),
            prices: PriceCache::load(),
            logged_in: AtomicBool::new(false),
        });
        let (tx, events) = unbounded();
        let emitter = Emitter { tx, ctx };

        let (data, data_rx) = unbounded();
        let (inventory, inventory_rx) = unbounded();
        let (prices, prices_rx) = unbounded();
        let (market, market_rx) = unbounded();
        let (log, log_rx) = unbounded();
        let (presence, presence_rx) = unbounded();

        spawn("data", {
            let (s, e) = (shared.clone(), emitter.clone());
            move || data_worker::run(s, e, data_rx)
        });
        spawn("inventory", {
            let (s, e) = (shared.clone(), emitter.clone());
            move || inventory_worker::run(s, e, inventory_rx)
        });
        spawn("prices", {
            let (s, e) = (shared.clone(), emitter.clone());
            move || price_worker::run(s, e, prices_rx)
        });
        spawn("market", {
            let e = emitter.clone();
            move || market_worker::run(e, market_rx)
        });
        spawn("log-router", {
            let (s, e, inv) = (shared.clone(), emitter.clone(), inventory.clone());
            move || log_router::run(s, e, log_rx, inv)
        });

        spawn("presence", {
            let (s, e) = (shared.clone(), emitter.clone());
            move || presence_worker::run(s, e, presence_rx)
        });

        let combo = shared.settings.read().unwrap().riven_hotkey.clone();
        let hotkey = crate::hotkey::Hotkey::start(&combo, {
            let inventory = inventory.clone();
            move || {
                let _ = inventory.send(InventoryCmd::CheckRiven);
            }
        });

        Self { shared, events, data, inventory, prices, market, log, presence, hotkey }
    }

    pub fn data(&self, cmd: DataCmd) {
        let _ = self.data.send(cmd);
    }
    pub fn inventory(&self, cmd: InventoryCmd) {
        let _ = self.inventory.send(cmd);
    }
    pub fn prices(&self, cmd: PriceCmd) {
        let _ = self.prices.send(cmd);
    }
    pub fn market(&self, cmd: MarketCmd) {
        let _ = self.market.send(cmd);
    }
    /// Shutdown work that must finish before the process exits.
    pub fn on_exit(&self) {
        presence_worker::on_app_exit(&self.shared);
    }

    pub fn presence(&self, cmd: PresenceCmd) {
        let _ = self.presence.send(cmd);
    }
    pub fn log(&self, cmd: LogCmd) {
        let _ = self.log.send(cmd);
    }

    /// Lists several orders in sequence (bulk sell).
    pub fn create_orders(&self, orders: Vec<NewOrder>) {
        self.market(MarketCmd::CreateOrders(orders));
    }

    pub fn import_inventory(&self, path: PathBuf) {
        self.inventory(InventoryCmd::Import(path));
    }
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .unwrap_or_else(|e| panic!("spawning {name} worker: {e}"));
}
