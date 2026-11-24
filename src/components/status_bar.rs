//! Title strip and status bar (original: `Titlebar.svelte`, `StatusBar.svelte`).

use egui::{Color32, FontId, Frame, Margin, RichText, Stroke, Ui};
use wfhelper_core::config::settings::InventorySource;

use super::format;
use crate::assets;
use crate::components::ui;
use crate::ipc::{Backend, InventoryCmd};
use crate::stores::{self, AppState};
use crate::theme::{self, *};

/// Top strip: logo, app name, data status pill, background activity.
pub fn title_strip(ui: &mut Ui, state: &AppState) {
    ui.horizontal_centered(|ui| {
        ui.add_space(6.0);
        ui::icon(ui, assets::LOGO, 18.0);
        ui.label(RichText::new("WFHELPER").font(FontId::new(13.0, theme::display_bold())).color(TEXT()));
        let (dot, text) = match (&state.db, &state.loading) {
            (_, Some(l)) => (WARNING(), l.clone()),
            (Some(_), None) => {
                let t = state.world.as_ref().and_then(|w| w.fetched_at).map(format::local_time).unwrap_or_default();
                (POSITIVE(), format!("WF data OK · {}", t.split(' ').next_back().unwrap_or("")))
            }
            (None, None) => (NEGATIVE(), "WF data unavailable".into()),
        };
        Frame::new()
            .fill(BG_SURFACE())
            .stroke(Stroke::new(1.0, BORDER_STRONG()))
            .corner_radius(RADIUS_SM())
            .inner_margin(Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(RichText::new("●").size(9.0).color(dot));
                    ui.label(RichText::new(text).size(11.0).color(TEXT_SECONDARY()));
                });
            });
    });
}

pub fn show(ui: &mut Ui, state: &AppState, backend: &Backend) {
    let settings = stores::settings(backend);
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let small = |t: String| RichText::new(t).size(11.5).color(TEXT_MUTED());
        match &state.inventory {
            Some(inv) => {
                let age = inv.fetched_at.map(format::ago).unwrap_or_default();
                ui.label(small(format!(
                    "{} items loaded · {} · {age}",
                    format::int(state.rows.len() as u64),
                    inv.source.unwrap_or(settings.inventory_source).label()
                )));
            }
            None => {
                ui.label(small("No inventory loaded".into()));
            }
        }
        if settings.inventory_source == InventorySource::Game {
            if state.helper.running {
                ui.add(egui::Spinner::new().size(11.0));
                ui.label(small("Reading game…".into()));
            } else {
                if let Some(f) = state.helper.last_failure {
                    ui.label(RichText::new("⚠").color(WARNING())).on_hover_text(f.hint());
                }
                if ui.add(egui::Label::new(small("⟳ Refresh".into())).sense(egui::Sense::click())).on_hover_text("Read inventory from the game now").clicked() {
                    backend.inventory(InventoryCmd::RefreshNow);
                }
            }
        }
        let (dot, text) = match (&state.ee_log, state.logged_in) {
            (Some(_), true) => (POSITIVE(), "In game"),
            (Some(_), false) => (WARNING(), "EE.log found"),
            (None, _) => (NEGATIVE(), "EE.log not found"),
        };
        ui.label(RichText::new("●").size(9.0).color(dot));
        ui.label(small(text.into())).on_hover_text(
            state.ee_log.as_ref().map(|p| p.display().to_string()).unwrap_or("Set the EE.log path in Settings".into()),
        );
        if state.price_queue > 0 {
            ui.add(egui::Spinner::new().size(11.0));
            ui.label(small(format!("Pricing {} items", state.price_queue)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(11.0).color(Color32::from_gray(90)));
            if let Some(s) = &state.session {
                ui.label(small(format!("WFM: {}", s.ingame_name.as_deref().unwrap_or("signed in"))));
            }
            let unread = state.history.len();
            ui.label(small(format!("🔔 {unread}"))).on_hover_ui(|ui| {
                for l in state.history.iter().rev().take(10) {
                    ui.label(format!("{}  {}", l.at.format("%H:%M"), l.text));
                }
            });
        });
    });
}
