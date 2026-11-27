//! Persisted user settings (original: `stores/preferences.ts`, `config/runtime/*`).

use serde::{Deserialize, Serialize};

use super::paths;
use crate::services::atomic_file;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum InventorySource {
    /// Read from the running game with the built-in wf-api reader.
    Game,
    /// An `inventory.json` the user imported.
    ImportedJson,
    /// AlecaFrame's encrypted `lastData.dat` cache.
    AlecaFrame,
    /// Continue without inventory.
    #[default]
    None,
}

impl InventorySource {
    pub fn label(self) -> &'static str {
        match self {
            InventorySource::Game => "Read from the running game",
            InventorySource::ImportedJson => "Imported inventory JSON",
            InventorySource::AlecaFrame => "Imported AlecaFrame cache",
            InventorySource::None => "No inventory",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ValueScope {
    #[default]
    PrimePartsOnly,
    AllTradables,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub setup_complete: bool,
    pub inventory_source: InventorySource,
    /// Minutes between automatic game reads.
    pub helper_poll_minutes: u32,
    /// Overrides EE.log discovery.
    pub ee_log_path: Option<String>,
    pub theme: ThemeChoice,
    /// Original theme preset id ("default" = Orokin Gold, "midnight", "nord", …).
    pub theme_preset: String,
    /// "sharp" | "soft" | "round"
    pub corner_style: String,
    /// Interface language code ("en", "uk").
    pub language: String,
    /// Inventory shows rows instead of cards.
    pub inventory_rows: bool,
    /// Unveiled rivens as small cards instead of the riven card art.
    pub riven_compact: bool,
    /// The "check riven" hotkey ("Ctrl+Alt+R"). On Wayland the desktop owns
    /// the key: this is only the one asked for.
    pub riven_hotkey: String,
    /// The in-game overlay (a riven cycle's rolls over the game).
    pub overlay_enabled: bool,
    /// Monitor the overlay goes on (None: Wayland compositors pick the focused
    /// one; elsewhere the first).
    pub overlay_monitor: Option<String>,
    /// Dailies/weeklies checklist: task id → reset timestamp it was ticked in.
    pub checklist: std::collections::BTreeMap<String, i64>,
    pub ui_scale: f32,
    pub landing_view: String,
    pub hidden_views: Vec<String>,

    pub notify_whispers: bool,
    pub notify_trades: bool,
    pub notify_arbitrations: bool,
    pub notify_fissures: bool,
    /// Arbitration nodes (`SolNode…`) that raise a notification when scheduled.
    pub arbi_favorites: Vec<String>,
    /// Planet cycles ("Earth", "Duviri", …) that notify when they change.
    pub cycle_alerts: Vec<String>,
    /// Also notify this many minutes before an alerted cycle changes (0: only on the change).
    pub cycle_alert_minutes: u32,
    /// Fissure rules: mission type labels that notify (Steel Path filter below).
    pub fissure_alert_missions: Vec<String>,
    pub fissure_alert_steel_path: bool,

    pub value_scope: ValueScope,
    pub value_min_plat: u32,
    /// Copies of each part kept back from "safe to sell".
    pub keep_spare_copies: u32,

    pub wfm_email: Option<String>,
    /// warframe.market status "In game" from logging in to Warframe until it
    /// closes (then put back), and "Invisible" when the app quits.
    pub wfm_auto_status: bool,
    /// The in-game name from EE.log, kept for when the game isn't running
    /// (Helminth invigorations are seeded with it).
    pub player_name: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            setup_complete: false,
            inventory_source: InventorySource::None,
            helper_poll_minutes: 10,
            ee_log_path: None,
            theme: ThemeChoice::Dark,
            theme_preset: "default".into(),
            corner_style: "soft".into(),
            language: "en".into(),
            inventory_rows: false,
            riven_compact: false,
            riven_hotkey: "Ctrl+Alt+R".into(),
            overlay_enabled: true,
            overlay_monitor: None,
            checklist: Default::default(),
            ui_scale: 1.0,
            landing_view: "inventory".into(),
            hidden_views: Vec::new(),
            notify_whispers: true,
            notify_trades: true,
            notify_arbitrations: true,
            notify_fissures: false,
            arbi_favorites: Vec::new(),
            cycle_alerts: Vec::new(),
            cycle_alert_minutes: 3,
            fissure_alert_missions: Vec::new(),
            fissure_alert_steel_path: false,
            value_scope: ValueScope::PrimePartsOnly,
            value_min_plat: 0,
            keep_spare_copies: 0,
            wfm_email: None,
            wfm_auto_status: true,
            player_name: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(paths::settings_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> crate::Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        atomic_file::write(&paths::settings_file(), text.as_bytes())
    }
}
