//! Arbitrations tab of World (original: `src/components/world/ArbiSchedule.svelte`,
//! `services/arbiSchedule.ts`). The run analysis and post-run summary are not ported.

use std::collections::BTreeMap;

use chrono::{Local, Utc};
use egui::{Align, Layout, Margin, RichText, Stroke, Ui};

use crate::components::{format, ui as kit};
use crate::ipc::Backend;
use crate::stores::{self, AppState};
use crate::theme::{self, *};
use crate::i18n::{tr, trf, trn};

pub struct ArbiView {
    days: i64,
    node_search: String,
    favorites_only: bool,
}

impl Default for ArbiView {
    fn default() -> Self {
        Self { days: 7, node_search: String::new(), favorites_only: false }
    }
}

impl ArbiView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        let (Some(arbi), Some(db)) = (state.arbi.clone(), state.db.clone()) else {
            ui.spinner();
            return;
        };
        let settings = stores::settings(backend);
        let now = Utc::now();
        let upcoming: Vec<_> = arbi
            .current(now)
            .into_iter()
            .chain(arbi.upcoming(now, (self.days * 24) as usize))
            .collect();
        let mut favorites = settings.arbi_favorites.clone();

        // Left: every node in the rotation, with favorites.
        egui::Panel::left("arbi-nodes")
            .resizable(false)
            .exact_size(280.0)
            .frame(egui::Frame::new().inner_margin(Margin { left: 0, right: 12, top: 0, bottom: 0 }))
            .show(ui, |ui| {
                let mut nodes: BTreeMap<String, String> = BTreeMap::new();
                for s in &arbi.slots {
                    nodes.entry(format::node(&db.node_label(&s.node_id))).or_insert_with(|| s.node_id.clone());
                }
                ui.horizontal(|ui| {
                    kit::caption(ui, tr("Nodes"));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(nodes.len().to_string()).size(11.5).color(TEXT_MUTED()));
                    });
                });
                kit::search(ui, &mut self.node_search, tr("Search nodes..."), ui.available_width());
                let q = self.node_search.trim().to_lowercase();
                egui::ScrollArea::vertical().max_height(ui.available_height() - 40.0).show(ui, |ui| {
                    for (label, id) in nodes.iter().filter(|(l, _)| q.is_empty() || l.to_lowercase().contains(&q)) {
                        let r = db.regions.get(id);
                        egui::Frame::new().inner_margin(Margin::symmetric(6, 4)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                let mut on = favorites.contains(id);
                                if ui.checkbox(&mut on, "").changed() {
                                    favorites.retain(|f| f != id);
                                    if on {
                                        favorites.push(id.clone());
                                    }
                                }
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.label(label);
                                    ui.label(RichText::new(crate::i18n::tr_any(r.map(|r| r.mission_type.as_str()).unwrap_or(""))).size(11.5).color(TEXT_MUTED()));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let faction = r.map(|r| r.faction.as_str()).unwrap_or("");
                                    ui.label(RichText::new("●").size(10.0).color(theme::faction_color(faction))).on_hover_text(faction);
                                });
                            });
                        });
                    }
                });
                ui.columns(2, |c| {
                    if c[0].add_sized([c[0].available_width(), 26.0], egui::Button::new(tr("All"))).clicked() {
                        favorites = nodes.values().cloned().collect();
                    }
                    if c[1].add_sized([c[1].available_width(), 26.0], egui::Button::new(tr("None"))).clicked() {
                        favorites.clear();
                    }
                });
            });

        // Right: schedule grouped by day.
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("arbi-days").width(80.0).selected_text(trn("{} day", "{} days", (self.days) as u64, &[&self.days])).show_ui(ui, |ui| {
                for d in [1, 3, 7] {
                    ui.selectable_value(&mut self.days, d, trn("{} day", "{} days", (d) as u64, &[&d]));
                }
            });
            ui.label(RichText::new(trf("Entries: {}", &[&upcoming.len()])).size(12.0).color(TEXT_MUTED()));
            if kit::chip(ui, self.favorites_only, tr("★ Favorites only")).clicked() {
                self.favorites_only = !self.favorites_only;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut notify = settings.notify_arbitrations;
                if ui.checkbox(&mut notify, tr("Notify 5 min before favorites")).changed() {
                    stores::update_settings(backend, |s| s.notify_arbitrations = notify);
                }
            });
        });
        ui.add_space(4.0);
        egui::Frame::new().fill(BG_SURFACE()).stroke(Stroke::new(1.0, BORDER_STRONG())).corner_radius(RADIUS_MD()).inner_margin(Margin::same(8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let header = |ui: &mut Ui| {
                ui.horizontal(|ui| {
                    for (t, w) in [(tr("Time"), 90.0), (tr("Node"), 300.0), (tr("Mission"), 220.0), (tr("Faction"), 140.0)] {
                        ui.allocate_ui(egui::vec2(w, 16.0), |ui| {
                            ui.set_width(w);
                            kit::caption(ui, t);
                        });
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        kit::caption(ui, tr("Starts in"));
                    });
                });
            };
            header(ui);
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let mut day = String::new();
                for slot in &upcoming {
                    let fav = favorites.contains(&slot.node_id);
                    if self.favorites_only && !fav {
                        continue;
                    }
                    let local = slot.start.with_timezone(&Local);
                    let d = local.format("%A, %b %-d").to_string().to_uppercase();
                    if d != day {
                        day = d.clone();
                        egui::Frame::new().fill(BG_RAISED()).inner_margin(Margin::symmetric(6, 3)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(RichText::new(d).size(11.5).family(theme::body_strong()).color(TEXT_SECONDARY()));
                        });
                    }
                    let r = db.regions.get(&slot.node_id);
                    let live = slot.start <= now;
                    let fill = if live { alpha(ACCENT(), 0.08) } else if fav { alpha(ACCENT(), 0.04) } else { egui::Color32::TRANSPARENT };
                    egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(6, 5)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let cell = |ui: &mut Ui, w: f32, text: RichText| {
                                ui.allocate_ui(egui::vec2(w, 18.0), |ui| {
                                    ui.set_width(w);
                                    ui.label(text);
                                });
                            };
                            cell(ui, 90.0, RichText::new(local.format("%I:%M %p").to_string()).size(12.5).color(TEXT_MUTED()));
                            cell(ui, 300.0, RichText::new(format::node(&db.node_label(&slot.node_id))).family(theme::body_strong()));
                            cell(ui, 220.0, RichText::new(crate::i18n::tr_any(r.map(|r| r.mission_type.as_str()).unwrap_or(""))).color(TEXT_SECONDARY()));
                            let faction = r.map(|r| r.faction.as_str()).unwrap_or("");
                            ui.allocate_ui(egui::vec2(140.0, 18.0), |ui| {
                                ui.set_width(140.0);
                                if !faction.is_empty() {
                                    kit::pill(ui, faction, theme::faction_color(faction));
                                }
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let star = if fav { RichText::new("★").color(ACCENT()) } else { RichText::new("☆").color(TEXT_MUTED()) };
                                if ui.add(egui::Label::new(star).sense(egui::Sense::click())).on_hover_text(tr("Favorite this node")).clicked() {
                                    favorites.retain(|f| f != &slot.node_id);
                                    if !fav {
                                        favorites.push(slot.node_id.clone());
                                    }
                                }
                                if live {
                                    ui.label(RichText::new(tr("NOW")).family(theme::body_strong()).color(POSITIVE()));
                                } else {
                                    ui.label(format::until(Some(slot.start)));
                                }
                            });
                        });
                    });
                }
            });
        });
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        if favorites != settings.arbi_favorites {
            stores::update_settings(backend, |s| s.arbi_favorites = favorites);
        }
    }
}
