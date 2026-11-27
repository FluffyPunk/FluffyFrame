//! FluffyFrame: unofficial Warframe companion, a Rust port of WFHelper.
//!
//! Original: `main.ts` (Electron bootstrap). Layout mirrors the original repo:
//! - `src/` here     ≈ original `src/` (views, components, stores) + `main.ts`
//! - `src/ipc/`      ≈ original `ipc/` + the Electron main process (worker threads)
//! - `crates/fluffyframe-core` ≈ original `config/` + `services/`
//! - `crates/wf-api` = FluffyPunk's wf-api-rust as a library (inventory from the game)

// Release builds on Windows run as a GUI app: no console window alongside it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod assets;
mod components;
mod console;
mod hotkey;
mod core_text;
mod i18n;
mod image_loader;
mod ipc;
mod memory;
mod riven_overlay;
mod stores;
mod theme;
mod theme_presets;
mod views;

use fluffyframe_core::config::app_meta;

fn main() -> eframe::Result<()> {
    console::attach_parent();
    memory::limit_arenas();
    fluffyframe_core::services::logger::init();
    log::info!("starting FluffyFrame {}", env!("CARGO_PKG_VERSION"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app_meta::APP_NAME)
            .with_app_id(app_meta::APP_ID)
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 560.0])
            // Frameless: the title bar is drawn by the app (status_bar::title_strip).
            .with_decorations(false),
        ..Default::default()
    };
    eframe::run_native(app_meta::APP_NAME, options, Box::new(|cc| Ok(Box::new(app::FluffyFrameApp::new(cc)))))
}
