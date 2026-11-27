//! First-run wizard (original: `src/views/SetupView.svelte`, `src/components/setup/*`).
//! The overlay placement step is gone with the overlays.

use egui::{FontId, RichText, Ui};
use fluffyframe_core::config::settings::InventorySource;

use super::ViewName;
use crate::assets;
use crate::components::ui as kit;
use crate::ipc::{Backend, InventoryCmd};
use crate::stores::{self, AppState};
use crate::theme::{self, *};
use crate::i18n::{tr, trf};

#[derive(Default)]
pub struct SetupView {
    step: usize,
}

impl SetupView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) -> Option<ViewName> {
        let mut s = stores::settings(backend);
        let mut done = None;
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            kit::icon(ui, assets::LOGO, 56.0);
            ui.label(RichText::new(tr("Welcome to FluffyFrame")).font(FontId::new(34.0, theme::display_bold())).color(TEXT()));
            ui.label(RichText::new(tr("Unofficial Warframe companion: inventory, prices, relics, world state.")).color(TEXT_SECONDARY()));
            ui.add_space(20.0);
            ui.set_max_width(780.0);
            kit::card(ui, |ui| {
                ui.set_width(740.0);
                ui.label(RichText::new(trf("Step {} of 2", &[&(self.step + 1)])).size(11.5).color(TEXT_MUTED()));
                match self.step {
                    0 => {
                        kit::card_title(ui, tr("Theme"));
                        if let Some(id) = kit::theme_picker(ui, &s.theme_preset) {
                            s.theme_preset = id;
                            stores::apply_theme(ui.ctx(), &s);
                        }
                        ui.add_space(6.0);
                        ui.label(tr("App size"));
                        if ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=1.75).step_by(0.05)).drag_stopped() {
                            stores::apply_theme(ui.ctx(), &s);
                        }
                        ui.add_space(12.0);
                        if kit::primary_button(ui, tr("   Next   ")).clicked() {
                            self.step = 1;
                        }
                    }
                    _ => {
                        kit::card_title(ui, tr("Choose an inventory source"));
                        ui.radio_value(&mut s.inventory_source, InventorySource::Game, tr("Read from the running game"));
                        ui.label(RichText::new(tr("Built in, nothing to install. Start Warframe and log in; refreshes every 10 minutes.")).size(12.0).color(TEXT_MUTED()));
                        #[cfg(target_os = "linux")]
                        ui.label(RichText::new(tr("Linux blocks reading another process's memory by default: `sudo setcap cap_sys_ptrace+ep <path to fluffyframe>`.")).size(12.0).color(TEXT_MUTED()));
                        ui.add_space(6.0);
                        if kit::ghost_button(ui, tr("Import inventory JSON or AlecaFrame lastData.dat…")).clicked()
                            && let Some(path) = rfd::FileDialog::new().add_filter("Inventory", &["json", "dat", "txt"]).pick_file()
                        {
                            backend.import_inventory(path);
                            s.inventory_source = InventorySource::ImportedJson;
                        }
                        ui.radio_value(&mut s.inventory_source, InventorySource::None, tr("Continue without inventory"));
                        ui.add_space(8.0);
                        if state.inventory.is_some() {
                            ui.label(RichText::new(tr("✔ Inventory loaded")).color(POSITIVE()));
                        }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if kit::ghost_button(ui, tr("Back")).clicked() {
                                self.step = 0;
                            }
                            if kit::primary_button(ui, tr("   Finish   ")).clicked() {
                                s.setup_complete = true;
                                done = Some(ViewName::from_id(&s.landing_view).unwrap_or(ViewName::Inventory));
                            }
                        });
                    }
                }
            });
        });
        let source_changed = s.inventory_source != stores::settings(backend).inventory_source;
        stores::update_settings(backend, |cur| *cur = s);
        if source_changed {
            backend.inventory(InventoryCmd::SourceChanged);
        }
        done
    }
}
