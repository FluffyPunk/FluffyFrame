//! Views (original: `src/views/*.svelte`, `src/lib/viewRegistry.ts`).
//!
//! Not ported: the in-game overlays, the OCR riven scanner and the arbitration
//! summary (excluded by request), plus Market Analytics and Missions/Enemies.

pub mod arbi;
mod dailies;
mod dashboard;
mod foundry;
mod inventory;
mod market;
mod mastery;
mod relics;
mod riven_finder;
mod rivens;
mod settings;
mod setup;
mod stats;
mod syndicates;
mod wiki;
mod world;

use egui::Ui;

use crate::ipc::Backend;
use crate::stores::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewName {
    Setup,
    Dashboard,
    Inventory,
    Foundry,
    Mastery,
    Stats,
    World,
    Syndicates,
    Market,
    Relics,
    Wiki,
    Rivens,
    Settings,
}

impl ViewName {
    /// Sidebar order (setup is wizard-only).
    pub const SIDEBAR: [ViewName; 12] = [
        ViewName::Dashboard,
        ViewName::Inventory,
        ViewName::Foundry,
        ViewName::Mastery,
        ViewName::Stats,
        ViewName::World,
        ViewName::Syndicates,
        ViewName::Market,
        ViewName::Relics,
        ViewName::Wiki,
        ViewName::Rivens,
        ViewName::Settings,
    ];

    pub fn id(self) -> &'static str {
        match self {
            ViewName::Setup => "setup",
            ViewName::Dashboard => "dashboard",
            ViewName::Inventory => "inventory",
            ViewName::Foundry => "foundry",
            ViewName::Mastery => "mastery",
            ViewName::Stats => "stats",
            ViewName::World => "world",
            ViewName::Syndicates => "syndicates",
            ViewName::Market => "market",
            ViewName::Relics => "relics",
            ViewName::Wiki => "wiki",
            ViewName::Rivens => "rivens",
            ViewName::Settings => "settings",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::SIDEBAR.into_iter().find(|v| v.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            ViewName::Setup => "Setup",
            ViewName::Dashboard => "Dashboard",
            ViewName::Inventory => "Inventory",
            ViewName::Foundry => "Foundry",
            ViewName::Mastery => "Mastery",
            ViewName::Stats => "Stats",
            ViewName::World => "World",
            ViewName::Syndicates => "Syndicates",
            ViewName::Market => "Market",
            ViewName::Relics => "Relics",
            ViewName::Wiki => "Wiki",
            ViewName::Rivens => "Rivens",
            ViewName::Settings => "Settings",
        }
    }

    /// Inventory and Settings can never be hidden.
    pub fn hideable(self) -> bool {
        !matches!(self, ViewName::Inventory | ViewName::Settings | ViewName::Setup)
    }
}

#[derive(Default)]
pub struct Views {
    dashboard: dashboard::DashboardView,
    inventory: inventory::InventoryView,
    foundry: foundry::FoundryView,
    mastery: mastery::MasteryView,
    stats: stats::StatsView,
    world: world::WorldView,
    syndicates: syndicates::SyndicatesView,
    market: market::MarketView,
    relics: relics::RelicsView,
    wiki: wiki::WikiView,
    rivens: rivens::RivensView,
    settings: settings::SettingsView,
    setup: setup::SetupView,
}

impl Views {
    /// Draws `view`; returns a view to switch to, if the view asked for one.
    pub fn show(&mut self, view: ViewName, ui: &mut Ui, state: &mut AppState, backend: &Backend) -> Option<ViewName> {
        match view {
            ViewName::Setup => return self.setup.ui(ui, state, backend),
            ViewName::Dashboard => return self.dashboard.ui(ui, state, backend),
            ViewName::Inventory => self.inventory.ui(ui, state, backend),
            ViewName::Foundry => self.foundry.ui(ui, state),
            ViewName::Mastery => self.mastery.ui(ui, state),
            ViewName::Stats => self.stats.ui(ui, state),
            ViewName::World => self.world.ui(ui, state, backend),
            ViewName::Syndicates => self.syndicates.ui(ui, state),
            ViewName::Market => self.market.ui(ui, state, backend),
            ViewName::Relics => self.relics.ui(ui, state, backend),
            ViewName::Wiki => self.wiki.ui(ui, state, backend),
            ViewName::Rivens => self.rivens.ui(ui, state, backend),
            ViewName::Settings => self.settings.ui(ui, state, backend),
        }
        None
    }
}

/// Shown by views that need game data before they can render.
pub fn need_game_data(ui: &mut Ui, state: &AppState) -> bool {
    if state.db.is_some() {
        return false;
    }
    crate::components::ui::empty(ui, "Loading game data…", state.loading.as_deref().unwrap_or("Fetching DE's public export"));
    true
}

/// Shown by views that need an inventory.
pub fn need_inventory(ui: &mut Ui, state: &AppState) -> bool {
    if need_game_data(ui, state) {
        return true;
    }
    if state.inventory.is_some() {
        return false;
    }
    crate::components::ui::empty(
        ui,
        "No inventory yet",
        "Start Warframe with the game source selected, or import an inventory in Settings.",
    );
    true
}
