//! Services, mirroring the original `services/` folder. Each module notes the
//! TypeScript file(s) it replaces.

// Infrastructure
pub mod atomic_file;
pub mod http;
pub mod image_cache;
pub mod json_cache;
pub mod logger;

// Game data
pub mod drop_data;
pub mod item_database;
pub mod market_blueprints;
pub mod public_export;
pub mod region_mastery;
pub mod riven_data;
pub mod riven_good_rolls;
pub mod wiki_art;

// Inventory
pub mod api_helper_runner;
pub mod inventory;
pub mod invigorations;
pub mod inventory_import;

// Derived inventory views
pub mod foundry;
pub mod full_sets;
pub mod mastery_helper;
pub mod relic_service;
pub mod rivens;

// warframe.market
pub mod wfm_catalog;
pub mod wfm_client;
pub mod wfm_orders;
pub mod wfm_presence;
pub mod wfm_riven_search;
pub mod wfm_stats_price;

// World
pub mod arbi_schedule;
pub mod bounty_cycle;
pub mod world_state;

// EE.log and trackers
#[cfg(windows)]
pub mod debug_output;
pub mod ee_log_monitor;
pub mod ee_log_path;
pub mod stats_tracker;
pub mod trade_tracker;

// Desktop integration
pub mod notifications;
pub mod warframe_status;
