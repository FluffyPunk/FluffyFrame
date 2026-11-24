//! WFHelper (Rust): unofficial Warframe companion.
//!
//! Original: `main.ts` (Electron bootstrap). Layout mirrors the original repo:
//! - `src/` here     ≈ original `src/` (views, components, stores) + `main.ts`
//! - `src/ipc/`      ≈ original `ipc/` + the Electron main process (worker threads)
//! - `crates/wfhelper-core` ≈ original `config/` + `services/`
//! - `crates/wf-api` = FluffyPunk's wf-api-rust as a library (inventory from the game)

mod app;
mod assets;
mod components;
mod image_loader;
mod ipc;
mod stores;
mod theme;
mod theme_presets;
mod views;

fn main() -> eframe::Result<()> {
    wfhelper_core::services::logger::init();
    log::info!("starting WFHelper {}", env!("CARGO_PKG_VERSION"));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("WFHelper")
            .with_app_id("wfhelper")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native("WFHelper", options, Box::new(|cc| Ok(Box::new(app::WfHelperApp::new(cc)))))
}
