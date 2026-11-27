//! Settings (original: `src/views/SettingsView.svelte`, `src/components/settings/*`).
//! Overlay, OCR and hotkey settings are gone along with those features.

use egui::{Align, FontId, Layout, RichText, Ui};
use fluffyframe_core::config::{app_meta, constants, paths};
use fluffyframe_core::services::riven_good_rolls;
use fluffyframe_core::config::settings::{InventorySource, Settings, ValueScope};

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

/// Page width cap (original: `max-w-[1320px]`, centered).
const MAX_WIDTH: f32 = 1320.0;
/// Panel layout (original `.settings-masonry`: `columns: 3 320px; column-gap: .85rem`).
const MAX_COLUMNS: usize = 3;
const MIN_COLUMN_WIDTH: f32 = 320.0;
const GAP: f32 = 13.0;

/// `add` in a column at most [`MAX_WIDTH`] wide, centered in the available width.
fn centered<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let avail = ui.available_rect_before_wrap();
    let w = avail.width().min(MAX_WIDTH);
    let rect = egui::Rect::from_min_size(egui::pos2(avail.center().x - w / 2.0, avail.top()), egui::vec2(w, avail.height()));
    let inner = ui.scope_builder(egui::UiBuilder::new().max_rect(rect), add);
    // Claim the full width so a surrounding scroll area keeps its scrollbar at
    // the window edge.
    ui.expand_to_include_rect(egui::Rect::from_min_max(avail.left_top(), egui::pos2(avail.right(), inner.response.rect.bottom())));
    inner.inner
}

/// As many columns as fit at [`MIN_COLUMN_WIDTH`] (1 to [`MAX_COLUMNS`]); each
/// panel goes into the currently shortest column, so the columns stay balanced.
struct Masonry<'a> {
    cols: &'a mut [Ui],
}

impl Masonry<'_> {
    fn count(width: f32) -> usize {
        (((width + GAP) / (MIN_COLUMN_WIDTH + GAP)).floor() as usize).clamp(1, MAX_COLUMNS)
    }

    fn panel(&mut self, title: &str, add: impl FnOnce(&mut Ui)) {
        let ui = self.cols.iter_mut().min_by(|a, b| a.cursor().top().total_cmp(&b.cursor().top())).expect("at least one column");
        kit::card(ui, |ui| {
            ui.set_width(ui.available_width());
            // Columns are justified; panel contents are not (wrapped labels
            // would be stretched to the full width, buttons too).
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.label(RichText::new(title).font(FontId::new(17.0, theme::display_bold())));
                ui.add_space(6.0);
                add(ui);
            });
        });
        ui.add_space(GAP);
    }
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

        centered(ui, |ui| {
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
        });

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            centered(ui, |ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                ui.columns(Masonry::count(ui.available_width()), |cols| {
                    let m = &mut Masonry { cols };
                    match self.tab {
                        Tab::General => self.general(m, state, backend, &mut s),
                        Tab::Notifications => self.notifications(m, &mut s),
                        Tab::Inventory => self.inventory(m, state, backend, &mut s),
                        Tab::Appearance => self.appearance(m, &mut s),
                        Tab::About => self.about(m),
                    }
                });
            });
        });

        if s.theme_preset != before.theme_preset || s.corner_style != before.corner_style {
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

    fn general(&mut self, m: &mut Masonry, state: &AppState, backend: &Backend, s: &mut Settings) {
        m.panel("Game log", |ui| {
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
        m.panel("Data", |ui| {
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
        m.panel("Startup", |ui| {
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

    fn notifications(&mut self, m: &mut Masonry, s: &mut Settings) {
        m.panel("Desktop notifications", |ui| {
            toggle(ui, "In-game whispers", &mut s.notify_whispers);
            toggle(ui, "Completed trades", &mut s.notify_trades);
            toggle(ui, "Favorite arbitrations", &mut s.notify_arbitrations);
            ui.label(RichText::new("Pick favorite nodes in World › Arbitrations.").size(11.5).color(TEXT_MUTED()));
        });
        m.panel("Void fissure alerts", |ui| {
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

    fn inventory(&mut self, m: &mut Masonry, state: &AppState, backend: &Backend, s: &mut Settings) {
        m.panel("Inventory source", |ui| {
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
                ui.label(RichText::new("Reading game memory needs ptrace access: `sudo setcap cap_sys_ptrace+ep <path to fluffyframe>`.").size(11.5).color(TEXT_MUTED()));
            }
        });
        m.panel("Import & export", |ui| {
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
        m.panel("Value estimate", |ui| {
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

    fn appearance(&mut self, m: &mut Masonry, s: &mut Settings) {
        m.panel("Presets", |ui| {
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
        m.panel("Style", |ui| {
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
            row(ui, "Riven cards", |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    kit::segmented(ui, &mut s.riven_compact, &[(false, "Full"), (true, "Compact")]);
                });
            });
        });
        m.panel("App size", |ui| {
            ui.label("Interface scale");
            ui.label(RichText::new("Zooms the whole app.").size(11.5).color(TEXT_MUTED()));
            // Rescaling mid-drag moves the slider under the cursor: apply on
            // release, or right away for typed values and arrow keys.
            let resp = ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=1.75).step_by(0.05).show_value(true));
            if resp.drag_stopped() || (resp.changed() && !resp.dragged()) {
                stores::apply_theme(ui.ctx(), s);
            }
        });
        m.panel("Sidebar views", |ui| {
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

    fn about(&mut self, m: &mut Masonry) {
        m.panel(app_meta::APP_NAME, |ui| {
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label(RichText::new("Unofficial Warframe companion. A Rust port of WFHelper; inventory reading by wf-api-rust.").color(TEXT_SECONDARY()));
            ui.add_space(4.0);
            ui.hyperlink_to("Source code", app_meta::REPO_URL);
            ui.hyperlink_to("WFHelper (original project)", app_meta::ORIGINAL_REPO_URL);
            ui.hyperlink_to("wf-api-rust", "https://github.com/FluffyPunk/wf-api-rust");
        });
        m.panel("Data sources", |ui| {
            let sheet = riven_good_rolls::sheet_url();
            let sources: [(&str, &str, &str); 8] = [
                ("DE Public Export", constants::PUBLIC_EXPORT_INDEX_URL, "Items, recipes, relics, riven stats"),
                ("DE world state", constants::WORLD_STATE_URL, "Fissures, events, Baro"),
                ("warframe.market", constants::WFM_SITE, "Prices, orders, riven auctions"),
                ("browse.wf", constants::ARBITRATION_SCHEDULE_URL, "Arbitration schedule"),
                ("drops.warframestat.us", constants::DROPS_URL, "Drop tables"),
                ("Warframe Wiki", constants::WIKI_BASE, "Item pages"),
                ("WFHelper assets", constants::ICON_MIRROR_BASE, "Mod and arcane card art"),
                ("Riven good rolls", &sheet, "Community stat recommendations"),
            ];
            for (name, url, what) in sources {
                ui.horizontal(|ui| {
                    ui.hyperlink_to(name, url).on_hover_text(url);
                    ui.label(RichText::new(what).size(12.0).color(TEXT_MUTED()));
                });
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Not affiliated with Digital Extremes.").size(11.5).color(TEXT_MUTED()));
        });
    }
}
