//! UI-side state (original: `src/stores/*`). Updated from worker [`Event`]s.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use fluffyframe_core::config::settings::Settings;
use fluffyframe_core::services::api_helper_runner::HelperStatus;
use fluffyframe_core::services::arbi_schedule::ArbiSchedule;
use fluffyframe_core::services::drop_data::DropIndex;
use fluffyframe_core::services::ee_log_monitor::LogEvent;
use fluffyframe_core::services::inventory::{self, Inventory, InventoryGroup, InventoryRow, relic_refinement};
use fluffyframe_core::services::item_database::ItemDb;
use fluffyframe_core::services::stats_tracker::StatsHistory;
use fluffyframe_core::services::trade_tracker::TradeLog;
use fluffyframe_core::services::wfm_catalog::Catalog;
use fluffyframe_core::services::wfm_client::Session;
use fluffyframe_core::services::wfm_orders::{Order, TopOrders};
use fluffyframe_core::services::wfm_riven_search::{RivenAuction, RivenWeapons};
use fluffyframe_core::services::wfm_stats_price::{Price, Variant, price_key};
use fluffyframe_core::services::world_state::WorldState;

use crate::ipc::{Backend, Event, PriceCmd, ToastKind};

pub struct Toast {
    pub kind: ToastKind,
    pub text: String,
    pub at: Instant,
}

/// One line of the notification history (original: `NotificationHistory.svelte`).
pub struct LogLine {
    pub at: chrono::DateTime<chrono::Local>,
    pub text: String,
}

#[derive(Default)]
pub struct AppState {
    pub db: Option<Arc<ItemDb>>,
    pub catalog: Option<Arc<Catalog>>,
    pub inventory: Option<Arc<Inventory>>,
    /// Classified inventory rows, rebuilt when inventory, db or catalog change.
    pub rows: Vec<InventoryRow>,
    pub prices: HashMap<String, Price>,
    pub price_queue: usize,
    pub world: Option<Arc<WorldState>>,
    pub arbi: Option<Arc<ArbiSchedule>>,
    pub drops: Option<Arc<DropIndex>>,
    pub drops_requested: bool,
    pub bounties: Option<Arc<fluffyframe_core::services::bounty_cycle::BountyBook>>,
    /// A bounty load is in flight.
    pub bounties_requested: bool,
    /// Relic families absent from the live drop tables; `None` until drops load.
    pub vaulted_relics: Option<HashSet<String>>,
    pub helper: HelperStatus,
    pub ee_log: Option<PathBuf>,
    pub logged_in: bool,
    pub trades: TradeLog,
    pub stats: StatsHistory,
    pub session: Option<Session>,
    pub my_orders: Vec<Order>,
    /// The user's open riven listings ("contracts").
    pub my_auctions: Vec<RivenAuction>,
    /// warframe.market status (None: signed out or unknown).
    pub wfm_status: Option<fluffyframe_core::services::wfm_presence::WfmStatus>,
    pub top_orders: HashMap<String, TopOrders>,
    pub riven_weapons: Option<Arc<RivenWeapons>>,
    pub riven_weapons_requested: bool,
    pub good_rolls: Option<Arc<fluffyframe_core::services::riven_good_rolls::GoodRolls>>,
    pub riven_auctions: HashMap<String, Vec<RivenAuction>>,
    pub loading: Option<String>,
    pub toasts: Vec<Toast>,
    pub history: Vec<LogLine>,
    /// Component or blueprint → the equipment it builds (for "Part of", pills).
    pub parents: HashMap<String, String>,
    /// unique name → owned count, rebuilt with the rows.
    pub counts: HashMap<String, u64>,
    /// Masterable items at max level.
    pub mastered: HashSet<String>,
    /// Item detail modal navigation stack (unique names); empty when closed.
    pub item_stack: Vec<String>,
}

/// Component/blueprint → equipment result, built once per game data load.
fn parent_index(db: &ItemDb) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for r in db.recipes.values() {
        if !db.get(&r.result_type).is_some_and(|i| i.kind.is_equipment()) {
            continue;
        }
        map.insert(r.unique_name.clone(), r.result_type.clone());
        for ing in &r.ingredients {
            map.entry(ing.item_type.clone()).or_insert_with(|| r.result_type.clone());
            if let Some(bp) = db.recipe_by_result.get(&ing.item_type) {
                map.entry(bp.clone()).or_insert_with(|| r.result_type.clone());
            }
        }
    }
    map
}

impl AppState {
    pub fn new() -> Self {
        Self { trades: TradeLog::load(), stats: StatsHistory::load(), ..Default::default() }
    }

    /// Opens the item detail modal (original: `ItemDetailModal.svelte`).
    pub fn open_item(&mut self, unique: impl Into<String>) {
        let unique = unique.into();
        if self.item_stack.last() != Some(&unique) {
            self.item_stack.push(unique);
        }
    }

    pub fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        let text = text.into();
        self.push_history(text.clone());
        self.toasts.push(Toast { kind, text, at: Instant::now() });
    }

    fn push_history(&mut self, text: String) {
        self.history.push(LogLine { at: chrono::Local::now(), text });
        if self.history.len() > 200 {
            self.history.remove(0);
        }
    }

    pub fn apply(&mut self, event: Event, backend: &Backend) {
        match event {
            Event::Loading(l) => self.loading = l,
            Event::GameData(db) => {
                self.parents = parent_index(&db);
                self.db = Some(db);
                self.recompute_vaulted();
                self.rebuild_rows(backend);
            }
            Event::Catalog(c) => {
                self.catalog = Some(c);
                self.rebuild_rows(backend);
            }
            Event::Inventory(inv) => {
                self.stats.record(&inv.currencies, chrono::Local::now());
                self.stats.save();
                self.inventory = Some(inv);
                self.rebuild_rows(backend);
            }
            Event::Helper(h) => self.helper = h,
            Event::World(w) => self.world = Some(w),
            Event::Arbi(a) => self.arbi = Some(a),
            Event::Drops(d) => {
                self.drops = Some(d);
                self.drops_requested = true;
                self.recompute_vaulted();
                self.rebuild_rows(backend);
            }
            Event::Bounties(b) => {
                self.bounties = Some(b);
                self.bounties_requested = false;
            }
            Event::Prices(list) => self.prices.extend(list),
            Event::PriceQueue(n) => self.price_queue = n,
            Event::TopOrders { key, orders } => {
                self.top_orders.insert(key, orders);
            }
            Event::MyOrders(o) => self.my_orders = o,
            Event::MyAuctions(a) => self.my_auctions = a,
            Event::WfmStatus(s) => self.wfm_status = s,
            Event::Session(s) => self.session = s,
            Event::RivenWeapons(w) => self.riven_weapons = Some(w),
            Event::GoodRolls(g) => self.good_rolls = Some(g),
            Event::RivenAuctions { slug, auctions } => {
                self.riven_auctions.insert(slug, auctions);
            }
            Event::Log(ev) => match ev {
                LogEvent::Attached(p) => self.ee_log = p,
                LogEvent::LoginComplete => self.logged_in = true,
                LogEvent::GameStarted => self.logged_in = false,
                LogEvent::Whisper(from) => self.push_history(format!("Whisper from {from}")),
                LogEvent::MissionEnded | LogEvent::TradeCompleted(_) | LogEvent::InventorySynced | LogEvent::PurchaseCompleted => {}
            },
            Event::Trade(t) => {
                self.push_history(format!("Trade with {}: {}p", t.trade.partner, t.trade.platinum));
                self.trades.events.push(t);
            }
            Event::Toast(kind, text) => self.toast(kind, text),
        }
    }

    fn recompute_vaulted(&mut self) {
        let (Some(db), Some(drops)) = (&self.db, &self.drops) else { return };
        let vaulted = db
            .relics
            .values()
            .filter(|r| r.refinement == fluffyframe_core::services::item_database::Refinement::Intact)
            .filter(|r| drops.lookup(&format!("{} Relic", r.name)).is_empty())
            .map(|r| r.family.clone())
            .collect();
        self.vaulted_relics = Some(vaulted);
    }

    /// Whether a relic family is vaulted; `None` while drop tables load.
    pub fn relic_vaulted(&self, family: &str) -> Option<bool> {
        self.vaulted_relics.as_ref().map(|v| v.contains(family))
    }

    fn rebuild_rows(&mut self, backend: &Backend) {
        let (Some(inv), Some(db)) = (&self.inventory, &self.db) else { return };
        let empty = Catalog::default();
        let catalog = self.catalog.as_deref().unwrap_or(&empty);
        self.rows = inventory::classify(inv, db, catalog);
        self.counts = inv.count_map().into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        self.mastered = db
            .items
            .values()
            .filter(|i| i.masterable)
            .filter(|i| {
                let xp = inv.xp_info.get(&i.unique_name).copied().unwrap_or(0);
                inventory::level_from_xp(xp, i.kind.is_frame_like(), i.max_level) >= i.max_level
            })
            .map(|i| i.unique_name.clone())
            .collect();
        // A relic is vaulted when no mission drops it; a part when all its relics are.
        if let Some(vaulted) = &self.vaulted_relics {
            let mut sources: HashMap<&str, Vec<&str>> = HashMap::new();
            for r in db.relics.values().filter(|r| r.refinement == fluffyframe_core::services::item_database::Refinement::Intact) {
                for w in &r.rewards {
                    sources.entry(w.unique_name.as_str()).or_default().push(r.family.as_str());
                }
            }
            for row in &mut self.rows {
                row.vaulted = match db.relics.get(&row.unique_name) {
                    Some(relic) => vaulted.contains(&relic.family),
                    None => {
                        let mut fams: Vec<&str> = sources.get(row.unique_name.as_str()).cloned().unwrap_or_default();
                        if let Some(bp) = db.recipe_by_result.get(&row.unique_name) {
                            fams.extend(sources.get(bp.as_str()).cloned().unwrap_or_default());
                        }
                        !fams.is_empty() && fams.iter().all(|f| vaulted.contains(*f))
                    }
                };
            }
        }
        // Price every tradable row in the background.
        let wanted: Vec<(String, Variant)> = self.rows.iter().filter_map(|r| Some((r.wfm_slug.clone()?, r.variant()))).collect();
        if !wanted.is_empty() {
            backend.prices(PriceCmd::Request(wanted));
        }
    }

    pub fn price(&self, slug: &str, variant: Variant) -> Option<f64> {
        self.prices.get(&price_key(slug, variant)).map(|p| p.average)
    }

    pub fn row_price(&self, row: &InventoryRow) -> Option<f64> {
        self.price(row.wfm_slug.as_deref()?, row.variant())
    }

    /// Market slug and pool for a DE unique name: relic refinements map to
    /// their subtype, ranked items to rank 0.
    pub fn market_ref(&self, unique: &str) -> Option<(String, Variant)> {
        let catalog = self.catalog.as_ref()?;
        if let Some((base, subtype)) = relic_refinement(unique) {
            return Some((catalog.by_game_ref(base)?.slug.clone(), Variant::Subtype(subtype)));
        }
        let item = catalog.by_game_ref(unique)?;
        Some((item.slug.clone(), if item.max_rank.is_some() { Variant::Rank(0) } else { Variant::Base }))
    }

    /// Price of an item by DE unique name (unranked).
    pub fn price_of_unique(&self, unique: &str) -> Option<f64> {
        let (slug, variant) = self.market_ref(unique)?;
        self.price(&slug, variant)
    }

    /// Asks the price worker for these unique names.
    pub fn request_prices_for(&self, backend: &Backend, uniques: impl IntoIterator<Item = String>) {
        let wanted: Vec<_> = uniques
            .into_iter()
            .filter_map(|u| self.market_ref(&u))
            .filter(|(slug, variant)| self.price(slug, *variant).is_none())
            .collect();
        if !wanted.is_empty() {
            backend.prices(PriceCmd::Request(wanted));
        }
    }

    /// The equipment `unique` belongs to, if it is a part.
    pub fn parent(&self, unique: &str) -> Option<&String> {
        self.parents.get(unique)
    }

    /// Owned count of `unique` (any rank) from the classified rows.
    pub fn owned(&self, unique: &str) -> u64 {
        self.counts.get(unique).copied().unwrap_or(0)
    }

    pub fn rows_in(&self, group: InventoryGroup) -> impl Iterator<Item = &InventoryRow> {
        self.rows
            .iter()
            .filter(move |r| group == InventoryGroup::Everything || r.group == group)
    }
}

/// Reads and writes settings shared with the workers, saving on change.
pub fn update_settings(backend: &Backend, f: impl FnOnce(&mut Settings)) {
    let mut s = backend.shared.settings.write().unwrap();
    f(&mut s);
    if let Err(e) = s.save() {
        log::warn!("saving settings: {e}");
    }
}

/// Applies the theme preset, corner style and app size from settings.
pub fn apply_theme(ctx: &egui::Context, s: &Settings) {
    let corners = match s.corner_style.as_str() {
        "sharp" => crate::theme::Corners::Sharp,
        "round" => crate::theme::Corners::Round,
        _ => crate::theme::Corners::Soft,
    };
    crate::theme::apply(ctx, &s.theme_preset, corners, s.ui_scale);
}

pub fn settings(backend: &Backend) -> Settings {
    backend.shared.settings.read().unwrap().clone()
}
