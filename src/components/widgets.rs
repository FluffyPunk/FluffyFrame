//! Link buttons (original: `WikiButton.svelte`, `DocsLink.svelte`).

use egui::Ui;
use fluffyframe_core::config::constants::WIKI_BASE;

pub fn wiki_button(ui: &mut Ui, name: &str) {
    let url = format!("{WIKI_BASE}{}", name.replace(' ', "_"));
    if ui.small_button("Wiki ↗").on_hover_text(&url).clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
    }
}

pub fn open_link(ui: &mut Ui, label: &str, url: &str) {
    if ui.small_button(format!("{label} ↗")).on_hover_text(url).clicked() {
        ui.ctx().open_url(egui::OpenUrl::new_tab(url.to_string()));
    }
}
