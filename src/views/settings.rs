//! Settings (original: `src/views/SettingsView.svelte`, `src/components/settings/*`).
//! Overlay, OCR and hotkey settings are gone along with those features.

use egui::{Align, FontId, Layout, RichText, Ui};
use wfhelper_core::config::paths;
use wfhelper_core::config::settings::{InventorySource, Settings, ValueScope};

use super::ViewName;
use crate::components::ui as kit;
use crate::ipc::{Backend, DataCmd, InventoryCmd, LogCmd};
use crate::stores::{self, AppState};
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    General,
    Notifications,
    Inventory,
    Appearance,
    About,
}

#[derive(Default)]
pub struct SettingsView {
    tab: Tab,
    ee_log_edit: Option<String>,
    fissure_mission: String,
}

fn panel(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    kit::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).font(FontId::new(17.0, theme::display_bold())));
        ui.add_space(6.0);
        add(ui);
    });
    ui.add_space(12.0);
}

/// Label on the left, control on the right.
fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.with_layout(Layout::right_to_left(Align::Center), add);
    });
    ui.add_space(2.0);
}

fn toggle(ui: &mut Ui, label: &str, value: &mut bool) {
    row(ui, label, |ui| {
        ui.checkbox(value, "");
    });
}

impl SettingsView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        let mut s = stores::settings(backend);
        let before = s.clone();

        ui.horizontal(|ui| {
            kit::page_title(ui, "Settings", None);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("Changes apply automatically.").size(12.0).color(TEXT_MUTED()));
            });
        });
        kit::tabs(
            ui,
            &mut self.tab,
            &[
                (Tab::General, "General".into()),
                (Tab::Notifications, "Notifications".into()),
                (Tab::Inventory, "Inventory & Mastery".into()),
                (Tab::Appearance, "Appearance".into()),
                (Tab::About, "About".into()),
            ],
        );

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.columns(3, |cols| match self.tab {
                Tab::General => self.general(cols, state, backend, &mut s),
                Tab::Notifications => self.notifications(cols, &mut s),
                Tab::Inventory => self.inventory(cols, state, backend, &mut s),
                Tab::Appearance => self.appearance(cols, &mut s),
                Tab::About => self.about(cols),
            });
        });

        if s.ui_scale != before.ui_scale || s.theme_preset != before.theme_preset || s.corner_style != before.corner_style {
            stores::apply_theme(ui.ctx(), &s);
        }
        let source_changed = s.inventory_source != before.inventory_source;
        let log_changed = s.ee_log_path != before.ee_log_path;
        if serde_json::to_string(&s).ok() != serde_json::to_string(&before).ok() {
            stores::update_settings(backend, |cur| *cur = s);
        }
        if source_changed {
            backend.inventory(InventoryCmd::SourceChanged);
        }
        if log_changed {
            backend.log(LogCmd::Restart);
        }
    }

    fn general(&mut self, cols: &mut [Ui], state: &AppState, backend: &Backend, s: &mut Settings) {
        panel(&mut cols[0], "Game log", |ui| {
            match &state.ee_log {
                Some(p) => {
                    kit::caption(ui, "Following");
                    ui.add(egui::Label::new(RichText::new(p.display().to_string()).size(12.0).color(TEXT_SECONDARY())).wrap_mode(egui::TextWrapMode::Truncate))
                        .on_hover_text(p.display().to_string());
                }
                None => {
                    ui.label(RichText::new("EE.log not found.").size(12.0).color(WARNING()));
                }
            }
            ui.add_space(4.0);
            let edit = self.ee_log_edit.get_or_insert_with(|| s.ee_log_path.clone().unwrap_or_default());
            ui.add(egui::TextEdit::singleline(edit).hint_text("Auto-detect").desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                if kit::ghost_button(ui, "Browse…").clicked()
                    && let Some(p) = rfd::FileDialog::new().add_filter("EE.log", &["log"]).pick_file()
                {
                    *edit = p.display().to_string();
                }
                if kit::ghost_button(ui, "Apply").clicked() {
                    s.ee_log_path = Some(edit.trim().to_string()).filter(|p| !p.is_empty());
                }
            });
            #[cfg(target_os = "linux")]
            ui.label(RichText::new("On Steam, `PROTON_LOG=1 %command%` in Warframe's launch options makes events arrive sooner.").size(11.5).color(TEXT_MUTED()));
        });
        panel(&mut cols[1], "Data", |ui| {
            ui.label(RichText::new(paths::data_dir().display().to_string()).size(12.0).color(TEXT_SECONDARY()));
            ui.horizontal(|ui| {
                if kit::ghost_button(ui, "Open data folder").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("file://{}", paths::data_dir().display())));
                }
                if kit::ghost_button(ui, "Reload game data").clicked() {
                    backend.data(DataCmd::ReloadGameData);
                }
            });
            ui.add_space(4.0);
            if kit::ghost_button(ui, "Run setup again").clicked() {
                s.setup_complete = false;
            }
        });
        panel(&mut cols[2], "Startup", |ui| {
            row(ui, "Open on", |ui| {
                egui::ComboBox::from_id_salt("landing")
                    .width(140.0)
                    .selected_text(ViewName::from_id(&s.landing_view).unwrap_or(ViewName::Inventory).label())
                    .show_ui(ui, |ui| {
                        for v in ViewName::SIDEBAR {
                            ui.selectable_value(&mut s.landing_view, v.id().to_string(), v.label());
                        }
                    });
            });
        });
    }

    fn notifications(&mut self, cols: &mut [Ui], s: &mut Settings) {
        panel(&mut cols[0], "Desktop notifications", |ui| {
            toggle(ui, "In-game whispers", &mut s.notify_whispers);
            toggle(ui, "Completed trades", &mut s.notify_trades);
            toggle(ui, "Favorite arbitrations", &mut s.notify_arbitrations);
            ui.label(RichText::new("Pick favorite nodes in World › Arbitrations.").size(11.5).color(TEXT_MUTED()));
        });
        panel(&mut cols[1], "Void fissure alerts", |ui| {
            toggle(ui, "Notify for matching fissures", &mut s.notify_fissures);
            toggle(ui, "Steel Path only", &mut s.fissure_alert_steel_path);
            ui.horizontal_wrapped(|ui| {
                let mut remove = None;
                for (i, m) in s.fissure_alert_missions.iter().enumerate() {
                    if kit::chip(ui, true, &format!("{m}  ✕")).clicked() {
                        remove = Some(i);
                    }
                }
                if let Some(i) = remove {
                    s.fissure_alert_missions.remove(i);
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.fissure_mission).hint_text("Mission type, e.g. Survival").desired_width(180.0));
                if kit::ghost_button(ui, "Add").clicked() && !self.fissure_mission.trim().is_empty() {
                    s.fissure_alert_missions.push(self.fissure_mission.trim().to_string());
                    self.fissure_mission.clear();
                }
            });
        });
    }

    fn inventory(&mut self, cols: &mut [Ui], state: &AppState, backend: &Backend, s: &mut Settings) {
        panel(&mut cols[0], "Inventory source", |ui| {
            for source in [InventorySource::Game, InventorySource::ImportedJson, InventorySource::AlecaFrame, InventorySource::None] {
                ui.radio_value(&mut s.inventory_source, source, source.label());
            }
            if s.inventory_source == InventorySource::Game {
                ui.add_space(4.0);
                row(ui, "Minutes between reads", |ui| {
                    ui.add(egui::DragValue::new(&mut s.helper_poll_minutes).range(2..=120));
                });
                if kit::ghost_button(ui, "Read now").clicked() {
                    backend.inventory(InventoryCmd::RefreshNow);
                }
                if let Some(f) = state.helper.last_failure {
                    ui.label(RichText::new(f.hint()).size(12.0).color(WARNING()));
                }
                #[cfg(target_os = "linux")]
                ui.label(RichText::new("Reading game memory needs ptrace access: `sudo setcap cap_sys_ptrace+ep <path to wfhelper>`.").size(11.5).color(TEXT_MUTED()));
            }
        });
        panel(&mut cols[1], "Import & export", |ui| {
            if kit::ghost_button(ui, "Import inventory JSON / AlecaFrame cache…").clicked()
                && let Some(path) = rfd::FileDialog::new().add_filter("Inventory", &["json", "dat", "txt"]).pick_file()
            {
                backend.import_inventory(path);
            }
            ui.add_space(4.0);
            if state.inventory.is_some()
                && kit::ghost_button(ui, "Export inventory…").clicked()
                && let Some(path) = rfd::FileDialog::new().set_file_name("inventory.json").save_file()
            {
                backend.inventory(InventoryCmd::Export(path));
            }
            ui.label(RichText::new("Export writes indented JSON instead of the single long line DE sends.").size(11.5).color(TEXT_MUTED()));
        });
        panel(&mut cols[2], "Value estimate", |ui| {
            ui.radio_value(&mut s.value_scope, ValueScope::PrimePartsOnly, "Prime parts only");
            ui.radio_value(&mut s.value_scope, ValueScope::AllTradables, "All tradables");
            row(ui, "Ignore items below", |ui| {
                ui.add(egui::DragValue::new(&mut s.value_min_plat).range(0..=1000).suffix(" p"));
            });
            row(ui, "Spare copies kept", |ui| {
                ui.add(egui::DragValue::new(&mut s.keep_spare_copies).range(0..=20));
            });
        });
    }

    fn appearance(&mut self, cols: &mut [Ui], s: &mut Settings) {
        panel(&mut cols[0], "Presets", |ui| {
            let label = theme::preset_list().into_iter().find(|(id, ..)| *id == s.theme_preset).map(|(_, l, _)| l).unwrap_or("Orokin Gold");
            ui.horizontal(|ui| {
                ui.label(RichText::new("Theme").color(TEXT_MUTED()));
                ui.label(RichText::new(label).family(theme::body_strong()));
            });
            ui.add_space(6.0);
            if let Some(id) = kit::theme_picker(ui, &s.theme_preset) {
                s.theme_preset = id;
            }
            ui.add_space(6.0);
            if ui.add(egui::Button::new(RichText::new("Restore All Defaults").color(NEGATIVE())).stroke(egui::Stroke::new(1.0, NEGATIVE()))).clicked() {
                s.theme_preset = "default".into();
                s.corner_style = "soft".into();
                s.ui_scale = 1.0;
                s.inventory_rows = false;
            }
        });
        panel(&mut cols[1], "Style", |ui| {
            row(ui, "Corners", |ui| {
                let mut v = s.corner_style.clone();
                let opts: [(&str, &str); 3] = [("sharp", "Sharp"), ("soft", "Soft"), ("round", "Round")];
                let mut idx = opts.iter().position(|(k, _)| *k == v).unwrap_or(1);
                // right-to-left layout: show the control as one widget
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let options: Vec<(usize, &str)> = opts.iter().enumerate().map(|(i, (_, l))| (i, *l)).collect();
                    if kit::segmented(ui, &mut idx, &options) {
                        v = opts[idx].0.to_string();
                    }
                });
                s.corner_style = v;
            });
            row(ui, "Inventory layout", |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    kit::segmented(ui, &mut s.inventory_rows, &[(false, "Cards"), (true, "Rows")]);
                });
            });
            ui.label(RichText::new("How the inventory grid shows items.").size(11.5).color(TEXT_MUTED()));
        });
        panel(&mut cols[2], "App size", |ui| {
            ui.label("Interface scale");
            ui.label(RichText::new("Zooms the whole app.").size(11.5).color(TEXT_MUTED()));
            ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=1.75).step_by(0.05).show_value(true));
        });
        panel(&mut cols[2], "Sidebar views", |ui| {
            for v in ViewName::SIDEBAR.into_iter().filter(|v| v.hideable()) {
                let mut shown = !s.hidden_views.iter().any(|h| h == v.id());
                row(ui, v.label(), |ui| {
                    if ui.checkbox(&mut shown, "").changed() {
                        s.hidden_views.retain(|h| h != v.id());
                        if !shown {
                            s.hidden_views.push(v.id().to_string());
                        }
                    }
                });
            }
        });
    }

    fn about(&mut self, cols: &mut [Ui]) {
        panel(&mut cols[0], "WFHelper (Rust)", |ui| {
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label(RichText::new("Unofficial Warframe companion. Rust port of WFHelper; inventory reading by wf-api-rust.").color(TEXT_SECONDARY()));
            ui.hyperlink_to("Original project", "https://github.com/WFHelper/WFHelper");
            ui.hyperlink_to("wf-api-rust", "https://github.com/FluffyPunk/wf-api-rust");
        });
        panel(&mut cols[1], "Data sources", |ui| {
            for line in ["DE public export & world state", "warframe.market (prices, orders)", "browse.wf (arbitrations)", "drops.warframestat.us (drop tables)"] {
                ui.label(RichText::new(line).color(TEXT_SECONDARY()));
            }
            ui.label(RichText::new("Not affiliated with Digital Extremes.").size(11.5).color(TEXT_MUTED()));
        });
    }
}
