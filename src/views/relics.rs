//! Relic Planner (original: `src/views/RelicsView.svelte`, `RelicDetailModal.svelte`,
//! `src/lib/relic/*`). The in-game relic planner overlay is not ported; its EV
//! ranking is here.

use egui::{Align, Color32, FontId, Layout, Margin, RichText, Sense, Stroke, Ui, vec2};
use fluffyframe_core::services::item_database::{Rarity, RelicEra, RelicInfo, Refinement};
use fluffyframe_core::services::relic_service::{self, RelicValue};

use super::need_game_data;
use crate::assets;
use crate::components::{format, ui as kit};
use crate::ipc::{Backend, DataCmd};
use crate::stores::AppState;
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Sort {
    #[default]
    Default,
    Plat,
    Ducats,
    Count,
}

pub struct RelicsView {
    era: Option<RelicEra>,
    search: String,
    owned_only: bool,
    refinement: Refinement,
    sort: Sort,
    vault: Option<bool>,
    selected: Option<String>,
    requested: bool,
    modal_refinement: Refinement,
    squad: u32,
    /// Reward clicked in the modal, opened in the item modal next frame.
    open_item: Option<String>,
}

impl Default for RelicsView {
    fn default() -> Self {
        Self {
            era: None,
            search: String::new(),
            owned_only: true,
            refinement: Refinement::Intact,
            sort: Sort::Default,
            vault: None,
            selected: None,
            requested: false,
            modal_refinement: Refinement::Intact,
            squad: 1,
            open_item: None,
        }
    }
}

struct Entry<'a> {
    info: &'a RelicInfo,
    counts: [u64; 4],
    /// `None` until the drop tables load.
    vaulted: Option<bool>,
    value: RelicValue,
}

fn rarity_color(r: Rarity) -> Color32 {
    match r {
        Rarity::Common => RARITY_COMMON,
        Rarity::Uncommon => RARITY_UNCOMMON,
        Rarity::Rare => RARITY_RARE,
    }
}

impl RelicsView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        if need_game_data(ui, state) {
            return;
        }
        let Some(db) = state.db.clone() else { return };
        if let Some(u) = self.open_item.take() {
            state.open_item(u);
        }
        let owned = state.inventory.as_ref().map(|inv| relic_service::owned_relics(inv, &db)).unwrap_or_default();
        let intact = relic_service::intact_by_family(&db);

        if !self.requested && state.catalog.is_some() {
            self.requested = true;
            let uniques: Vec<String> = intact.values().flat_map(|r| r.rewards.iter().map(|w| w.unique_name.clone())).collect();
            state.request_prices_for(backend, uniques);
        }
        if self.selected.is_some() && state.drops.is_none() && !state.drops_requested {
            state.drops_requested = true;
            backend.data(DataCmd::LoadDrops);
        }

        let price_of = |u: &str| state.price_of_unique(u);
        let q = self.search.trim().to_lowercase();
        let mut entries: Vec<Entry> = intact
            .values()
            .filter_map(|info| {
                let o = owned.iter().find(|o| o.family == info.family);
                if self.owned_only && o.is_none() {
                    return None;
                }
                let vaulted = state.relic_vaulted(&info.family);
                Some(Entry {
                    info,
                    counts: o.map(|o| o.counts).unwrap_or([0; 4]),
                    vaulted,
                    value: relic_service::expected_value(info, self.refinement, &db, &price_of),
                })
            })
            .filter(|e| e.info.era != RelicEra::Other)
            .filter(|e| self.era.is_none_or(|era| era == e.info.era))
            .filter(|e| self.vault.is_none_or(|v| e.vaulted == Some(v)))
            .filter(|e| {
                q.is_empty()
                    || e.info.name.to_lowercase().contains(&q)
                    || e.info.rewards.iter().any(|w| db.name(&w.unique_name).to_lowercase().contains(&q))
            })
            .collect();
        entries.sort_by(|a, b| match self.sort {
            Sort::Default => (a.info.era, natural(&a.info.name)).cmp(&(b.info.era, natural(&b.info.name))),
            Sort::Plat => b.value.platinum.total_cmp(&a.value.platinum),
            Sort::Ducats => b.value.ducats.total_cmp(&a.value.ducats),
            Sort::Count => b.counts.iter().sum::<u64>().cmp(&a.counts.iter().sum::<u64>()),
        });

        ui.horizontal(|ui| {
            kit::page_title(ui, &format!("Relic Planner ({} relics)", entries.len()), None);
        });
        ui.horizontal(|ui| {
            let tabs: Vec<(Option<RelicEra>, String)> =
                std::iter::once((None, "All".to_string())).chain(RelicEra::ALL.iter().map(|e| (Some(*e), e.label().to_string()))).collect();
            ui.allocate_ui(vec2(470.0, 40.0), |ui| kit::tabs(ui, &mut self.era, &tabs));
            kit::search(ui, &mut self.search, "Search relics...", 180.0);
            kit::labelled(ui, "Relics", |ui| {
                egui::ComboBox::from_id_salt("relic-owned").width(110.0).selected_text(if self.owned_only { "Owned only" } else { "All relics" }).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.owned_only, true, "Owned only");
                    ui.selectable_value(&mut self.owned_only, false, "All relics");
                });
            });
            kit::labelled(ui, "Quality", |ui| {
                egui::ComboBox::from_id_salt("relic-ref").width(110.0).selected_text(self.refinement.label()).show_ui(ui, |ui| {
                    for r in Refinement::ALL {
                        ui.selectable_value(&mut self.refinement, r, r.label());
                    }
                });
            });
            kit::labelled(ui, "Vault", |ui| {
                egui::ComboBox::from_id_salt("relic-vault")
                    .width(100.0)
                    .selected_text(match self.vault {
                        None => "All",
                        Some(true) => "Vaulted",
                        Some(false) => "Unvaulted",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.vault, None, "All");
                        ui.selectable_value(&mut self.vault, Some(true), "Vaulted");
                        ui.selectable_value(&mut self.vault, Some(false), "Unvaulted");
                    });
            });
            kit::labelled(ui, "Sort", |ui| {
                egui::ComboBox::from_id_salt("relic-sort")
                    .width(110.0)
                    .selected_text(match self.sort {
                        Sort::Default => "Default",
                        Sort::Plat => "EV platinum",
                        Sort::Ducats => "EV ducats",
                        Sort::Count => "Copies",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.sort, Sort::Default, "Default");
                        ui.selectable_value(&mut self.sort, Sort::Plat, "EV platinum");
                        ui.selectable_value(&mut self.sort, Sort::Ducats, "EV ducats");
                        ui.selectable_value(&mut self.sort, Sort::Count, "Copies");
                    });
            });
        });
        ui.add_space(6.0);

        if entries.is_empty() {
            kit::empty(ui, "No relics", "No relics match these filters.");
        }
        let refinement = self.refinement;
        let mut open = None;
        kit::card_grid(ui, 300.0, 150.0, entries.len(), |ui, i| {
            let e = &entries[i];
            let resp = kit::grid_card(ui, None, |ui| {
                egui::Frame::new().inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        kit::icon(ui, assets::relic_icon(e.info.era.label()), 30.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.label(RichText::new(&e.info.name).font(FontId::new(20.0, theme::display_bold())));
                            match e.vaulted {
                                Some(true) => kit::pill(ui, "Vaulted", NEGATIVE()),
                                Some(false) => kit::pill(ui, "Unvaulted", INFO()),
                                None => kit::pill(ui, "…", TEXT_MUTED()),
                            };
                        });
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            ui.vertical(|ui| {
                                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                    ui.label(RichText::new(format!("SELECTED EV: {}", refinement.label().to_uppercase())).size(9.5).color(TEXT_MUTED()));
                                });
                                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                    let dpp = if e.value.platinum > 0.0 { e.value.ducats / e.value.platinum } else { 0.0 };
                                    ui.label(RichText::new(format!("{dpp:.1}")).family(theme::body_strong()).color(ACCENT()));
                                    ui.label(RichText::new("d/p").size(10.5).color(TEXT_MUTED()));
                                    ui.label(RichText::new(format!("{:.1}", e.value.ducats)).family(theme::body_strong()).color(ACCENT()));
                                    kit::icon(ui, assets::DUCATS_SMALL, 12.0);
                                    ui.label(RichText::new(format!("{:.1}", e.value.platinum)).family(theme::body_strong()));
                                    kit::icon(ui, assets::PLATINUM, 12.0);
                                });
                            });
                        });
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let n = relic_service::sorted_rewards(e.info).len().max(1) as f32;
                        let tile = ((ui.available_width() - 4.0 * (n - 1.0)) / n).min(44.0);
                        for w in relic_service::sorted_rewards(e.info).into_iter().rev() {
                            let (rect, resp) = ui.allocate_exact_size(vec2(tile, tile), Sense::hover());
                            let c = rarity_color(w.rarity);
                            // Green when you already have it (or its finished item / mastery).
                            let have = state.owned(&w.unique_name) > 0
                                || state.parent(&w.unique_name).is_some_and(|p| state.mastered.contains(p) || state.owned(p) > 0);
                            let fill = if have { alpha(POSITIVE(), 0.16) } else { alpha(c, 0.10) };
                            ui.painter().rect(rect, RADIUS_SM(), fill, Stroke::new(1.0, alpha(c, 0.7)), egui::StrokeKind::Inside);
                            if let Some(url) = db.image_url(&w.unique_name) {
                                kit::paint_image_fit(ui, &url, rect.shrink(3.0));
                            }
                            resp.on_hover_text(format!(
                                "{}\n{} · {}\n{}",
                                db.name(&w.unique_name),
                                w.rarity.label(),
                                relic_service::rarity_chance_label(w.rarity, refinement),
                                format::plat(price_of(&w.unique_name))
                            ));
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let w = (ui.available_width() - 12.0) / 4.0;
                        for (i, label) in ["Int", "Ex", "Fl", "Rad"].iter().enumerate() {
                            let n = e.counts[i];
                            let (fill, stroke, color) = if n > 0 {
                                (alpha(ACCENT(), 0.18), ACCENT(), ACCENT_BRIGHT())
                            } else {
                                (BG_RAISED(), BORDER(), TEXT_MUTED())
                            };
                            egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, stroke)).corner_radius(RADIUS_SM()).show(ui, |ui| {
                                ui.set_width(w - 2.0);
                                ui.vertical_centered(|ui| ui.label(RichText::new(format!("{label}: {n}")).size(11.0).color(color)));
                            });
                        }
                    });
                });
            });
            if resp.clicked() {
                open = Some(e.info.family.clone());
            }
        });
        if open.is_some() {
            self.selected = open;
            self.modal_refinement = self.refinement;
        }
        self.detail(ui, state, &db, &intact);
    }

    /// Relic detail (original: `RelicDetailModal.svelte`).
    fn detail(&mut self, ui: &mut Ui, state: &AppState, db: &fluffyframe_core::services::item_database::ItemDb, intact: &std::collections::HashMap<&str, &RelicInfo>) {
        let Some(family) = self.selected.clone() else { return };
        let Some(info) = intact.get(family.as_str()) else { return };
        let price_of = |u: &str| state.price_of_unique(u);
        let counts = state.inventory.as_ref().map(|inv| relic_service::owned_relics(inv, db)).unwrap_or_default();
        let owned = counts.iter().find(|o| o.family == family).map(|o| o.counts).unwrap_or([0; 4]);
        let frame = egui::Frame::new().fill(BG_SURFACE()).stroke(Stroke::new(1.0, BORDER_STRONG())).corner_radius(RADIUS_LG()).inner_margin(Margin::same(16));
        let resp = egui::Modal::new(egui::Id::unique("relic-detail")).frame(frame).show(ui.ctx(), |ui| {
            ui.set_width(660.0);
            ui.horizontal(|ui| {
                egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_MD()).inner_margin(Margin::same(4)).show(ui, |ui| {
                    kit::icon(ui, assets::relic_icon(info.era.label()), 46.0);
                });
                ui.vertical(|ui| {
                    ui.label(RichText::new(&info.name).font(FontId::new(20.0, theme::display_bold())));
                    ui.horizontal(|ui| {
                        match state.relic_vaulted(&family) {
                            Some(true) => kit::pill(ui, "Vaulted", NEGATIVE()),
                            Some(false) => kit::pill(ui, "Unvaulted", INFO()),
                            None => kit::pill(ui, "…", TEXT_MUTED()),
                        };
                        for (i, r) in Refinement::ALL.iter().enumerate() {
                            if owned[i] > 0 {
                                kit::pill(ui, &format!("{}: x{}", r.label(), owned[i]), TEXT_SECONDARY());
                            }
                        }
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                    if ui.button("✕").clicked() {
                        self.selected = None;
                    }
                    crate::components::widgets::wiki_button(ui, &format!("{} Relic", info.name));
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for r in Refinement::ALL {
                    if kit::chip(ui, self.modal_refinement == r, r.label()).clicked() {
                        self.modal_refinement = r;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("Squad:").size(12.5).color(TEXT_MUTED()));
                for (n, l) in [(1, "Solo"), (2, "2P"), (3, "3P"), (4, "4P")] {
                    if kit::chip(ui, self.squad == n, l).clicked() {
                        self.squad = n;
                    }
                }
            });
            ui.add_space(6.0);
            let refinement = self.modal_refinement;
            let (total, per) = relic_service::squad_value(info, refinement, self.squad, db, &price_of);
            egui::Grid::new("relic-rewards").striped(true).num_columns(6).spacing([14.0, 7.0]).min_col_width(40.0).show(ui, |ui| {
                ui.label("");
                for h in ["Item", "Chance", "Price", "Ducats", "E.V."] {
                    kit::caption(ui, h);
                }
                ui.end_row();
                let mut order: Vec<usize> = (0..info.rewards.len()).collect();
                order.sort_by_key(|&i| info.rewards[i].rarity);
                for i in order {
                    let w = &info.rewards[i];
                    let (letter, color) = match w.rarity {
                        Rarity::Common => ("C", RARITY_COMMON),
                        Rarity::Uncommon => ("U", RARITY_UNCOMMON),
                        Rarity::Rare => ("R", RARITY_RARE),
                    };
                    let (rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                    ui.painter().circle(rect.center(), 9.0, alpha(color, 0.15), Stroke::new(1.0, color));
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, letter, FontId::new(10.5, theme::body_strong()), color);
                    ui.horizontal(|ui| {
                        let resp = ui.add(egui::Label::new(RichText::new(db.name(&w.unique_name))).sense(Sense::click()));
                        if resp.clicked() {
                            self.open_item = Some(w.unique_name.clone());
                        }
                        let parent = state.parent(&w.unique_name);
                        if parent.is_some_and(|p| state.mastered.contains(p)) {
                            kit::pill(ui, "Mastered", POSITIVE());
                        }
                        if parent.is_some_and(|p| state.owned(p) > 0) {
                            kit::pill(ui, "Parent owned", INFO());
                        }
                    });
                    ui.label(RichText::new(relic_service::rarity_chance_label(w.rarity, refinement)).size(12.5).color(TEXT_SECONDARY()));
                    match price_of(&w.unique_name) {
                        Some(p) => ui.label(RichText::new(format!("{p:.0}p")).family(theme::body_strong()).color(ACCENT_BRIGHT())),
                        None => ui.label(RichText::new("-").color(TEXT_MUTED())),
                    };
                    match db.get(&w.unique_name).and_then(|i| i.ducats) {
                        Some(d) => ui.label(RichText::new(format!("{d}d")).size(12.5)),
                        None => ui.label(RichText::new("-").color(TEXT_MUTED())),
                    };
                    let ev = per.get(i).copied().unwrap_or_default();
                    ui.label(RichText::new(format!("~{:.1}p | {:.1}d", ev.platinum, ev.ducats)).size(12.0).color(TEXT_SECONDARY()));
                    ui.end_row();
                }
            });
            ui.separator();
            let squad = match self.squad {
                1 => "Solo".to_string(),
                n => format!("{n}P"),
            };
            let dpp = if total.platinum > 0.0 { total.ducats / total.platinum } else { 0.0 };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("Expected value ({}, {squad}):", refinement.label())).color(TEXT_SECONDARY()));
                ui.label(RichText::new(format!("~{:.1}", total.platinum)).family(theme::body_strong()).color(ACCENT_BRIGHT()));
                ui.label(RichText::new("platinum |").color(TEXT_SECONDARY()));
                ui.label(RichText::new(format!("{:.1}", total.ducats)).family(theme::body_strong()).color(ACCENT_BRIGHT()));
                ui.label(RichText::new(format!("ducats ({dpp:.1} ducats/plat)")).color(TEXT_SECONDARY()));
            });
            ui.add_space(6.0);
            egui::CollapsingHeader::new(RichText::new("Where to farm").family(theme::body_strong())).show(ui, |ui| match &state.drops {
                None => {
                    ui.spinner();
                }
                Some(drops) => {
                    let sources = drops.lookup(&format!("{} Relic", info.name));
                    if sources.is_empty() {
                        ui.label(RichText::new("No current drop locations (vaulted).").color(TEXT_MUTED()));
                    }
                    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        for s in sources.iter().take(20) {
                            ui.horizontal(|ui| {
                                ui.label(&s.location);
                                ui.label(RichText::new(&s.detail).color(TEXT_MUTED()));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{:.2}%", s.chance)).family(theme::body_strong()));
                                });
                            });
                        }
                    });
                }
            });
        });
        if resp.should_close() {
            self.selected = None;
        }
    }
}

/// "Axi A10" sorts after "Axi A9".
fn natural(name: &str) -> (String, u32) {
    let code = name.split_whitespace().nth(1).unwrap_or("");
    let letters: String = code.chars().take_while(|c| c.is_alphabetic()).collect();
    let digits = code.chars().skip_while(|c| c.is_alphabetic()).collect::<String>().parse().unwrap_or(0);
    (letters, digits)
}
