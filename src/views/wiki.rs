//! Wiki / Drop Data (original: `src/views/WikiView.svelte`, `ItemDetailModal.svelte`,
//! `DropsList.svelte`).

use egui::{Align, FontId, Layout, Margin, RichText, Ui, vec2};
use fluffyframe_core::services::item_database::Refinement;
use fluffyframe_core::services::relic_service;

use super::need_game_data;
use crate::components::{format, ui as kit, widgets};
use crate::ipc::{Backend, DataCmd};
use crate::stores::AppState;
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Item,
    Location,
}

#[derive(Default)]
pub struct WikiView {
    search: String,
    mode: Mode,
    selected: Option<String>,
}

impl WikiView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        if need_game_data(ui, state) {
            return;
        }
        if state.drops.is_none() && !state.drops_requested {
            state.drops_requested = true;
            backend.data(DataCmd::LoadDrops);
        }
        let Some(db) = state.db.clone() else { return };

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = ui.available_width().min(1040.0);
            let pad = (ui.available_width() - width) / 2.0;
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    ui.add_space(10.0);
                    kit::page_title(ui, "Drop Data", None);
                    kit::page_subtitle(ui, "Search the full Warframe drop tables for any item's drop locations and rates.");
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let hint = if self.mode == Mode::Item { "Search an item (e.g. Forma)" } else { "Search a location (e.g. Hydron)" };
                        if kit::search(ui, &mut self.search, hint, width - 210.0).changed() {
                            self.selected = None;
                        }
                        if kit::chip(ui, self.mode == Mode::Item, "By item").clicked() {
                            self.mode = Mode::Item;
                        }
                        if kit::chip(ui, self.mode == Mode::Location, "By location").clicked() {
                            self.mode = Mode::Location;
                            self.selected = None;
                        }
                    });
                    ui.add_space(10.0);

                    if self.mode == Mode::Location {
                        self.locations(ui, state);
                        return;
                    }
                    if let Some(unique) = self.selected.clone() {
                        self.item(ui, state, backend, &db, &unique);
                        return;
                    }
                    if self.search.trim().len() < 2 {
                        kit::card(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| ui.label(RichText::new("Type to search drop tables.").color(TEXT_SECONDARY())));
                        });
                        return;
                    }
                    for item in db.search(&self.search, 60) {
                        let resp = kit::raised(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                kit::remote_image(ui, db.image_url(&item.unique_name).as_deref(), vec2(32.0, 32.0));
                                ui.label(RichText::new(&item.name).family(theme::body_strong()));
                                ui.label(RichText::new(item.kind.label()).size(12.0).color(TEXT_MUTED()));
                                if let Some(drops) = &state.drops {
                                    let n = drops.lookup(&item.name).len();
                                    if n > 0 {
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(RichText::new(format!("{n} sources")).size(12.0).color(TEXT_SECONDARY()));
                                        });
                                    }
                                }
                            });
                            ui.interact(ui.min_rect(), ui.next_auto_id(), egui::Sense::click())
                        });
                        if resp.clicked() {
                            self.selected = Some(item.unique_name.clone());
                            state.request_prices_for(backend, [item.unique_name.clone()]);
                        }
                        ui.add_space(3.0);
                    }
                });
            });
        });
    }

    fn locations(&mut self, ui: &mut Ui, state: &AppState) {
        let Some(drops) = &state.drops else {
            ui.spinner();
            return;
        };
        let hits = drops.by_location(&self.search, 200);
        if hits.is_empty() {
            kit::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| ui.label(RichText::new("Type a mission node, planet or enemy.").color(TEXT_SECONDARY())));
            });
            return;
        }
        let mut current = "";
        for (item, s) in hits {
            if s.location != current {
                current = &s.location;
                ui.add_space(6.0);
                kit::caption(ui, current);
            }
            ui.horizontal(|ui| {
                ui.label(crate::components::format::title_case(item));
                ui.label(RichText::new(&s.detail).size(12.0).color(TEXT_MUTED()));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.2}%", s.chance)).family(theme::body_strong()));
                });
            });
        }
    }

    fn item(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend, db: &fluffyframe_core::services::item_database::ItemDb, unique: &str) {
        let Some(item) = db.get(unique) else { return };
        if kit::ghost_button(ui, "‹ Back to results").clicked() {
            self.selected = None;
            return;
        }
        ui.add_space(6.0);
        kit::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_MD()).inner_margin(Margin::same(6)).show(ui, |ui| {
                    kit::remote_image(ui, db.image_url(unique).as_deref(), vec2(84.0, 84.0));
                });
                ui.vertical(|ui| {
                    ui.label(RichText::new(&item.name).font(FontId::new(26.0, theme::display_bold())));
                    ui.horizontal(|ui| {
                        kit::pill(ui, item.kind.label(), TEXT_SECONDARY());
                        if state.rows.iter().any(|r| r.unique_name == unique && r.vaulted) {
                            kit::pill(ui, "Vaulted", NEGATIVE());
                        }
                        if item.mastery_req > 0 {
                            kit::pill(ui, &format!("MR {}", item.mastery_req), ACCENT());
                        }
                        if let Some(inv) = &state.inventory {
                            kit::pill(ui, &format!("Owned {}", inv.count_of(unique)), POSITIVE());
                        }
                    });
                    ui.horizontal(|ui| {
                        widgets::wiki_button(ui, &item.name);
                        if let Some(c) = state.catalog.as_ref().and_then(|c| c.by_game_ref(unique)) {
                            widgets::open_link(ui, "warframe.market", &c.market_url());
                            kit::plat(ui, state.price_of_unique(unique), 13.0);
                        }
                        if item.ducats.is_some() {
                            kit::ducats(ui, item.ducats, 13.0);
                        }
                    });
                });
            });
            if !item.description.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&item.description).color(TEXT_SECONDARY()));
            }
        });
        ui.add_space(10.0);

        let recipe = db.recipes.get(unique).or_else(|| db.recipe_for_result(unique));
        let mut targets = vec![unique.to_string()];
        if let Some(bp) = db.recipe_by_result.get(unique) {
            targets.push(bp.clone());
        }
        let relics: Vec<_> = targets.iter().flat_map(|u| db.relics_dropping(u)).collect();

        ui.columns(2, |cols| {
            if let Some(recipe) = recipe {
                let ui = &mut cols[0];
                kit::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    kit::card_title(ui, "Recipe");
                    ui.label(RichText::new(format!("{} cr · {}", format::int(recipe.build_price), format::duration_secs(recipe.build_time_secs))).size(12.0).color(TEXT_MUTED()));
                    let counts = state.inventory.as_ref().map(|i| i.count_map()).unwrap_or_default();
                    for ing in &recipe.ingredients {
                        ui.horizontal(|ui| {
                            kit::remote_image(ui, db.image_url(&ing.item_type).as_deref(), vec2(26.0, 26.0));
                            if ui.link(db.name(&ing.item_type)).clicked() {
                                self.selected = Some(ing.item_type.clone());
                                state.request_prices_for(backend, [ing.item_type.clone()]);
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                kit::have_need(ui, counts.get(ing.item_type.as_str()).copied().unwrap_or(0), ing.count as u64);
                            });
                        });
                    }
                });
                cols[0].add_space(10.0);
            }
            if !relics.is_empty() {
                let ui = &mut cols[0];
                kit::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    kit::card_title(ui, "Relics");
                    for (relic, rarity) in &relics {
                        ui.horizontal(|ui| {
                            kit::icon(ui, crate::assets::relic_icon(relic.era.label()), 20.0);
                            ui.label(&relic.name);
                            kit::pill(ui, rarity.label(), TEXT_SECONDARY());
                            if state.relic_vaulted(&relic.family) == Some(true) {
                                kit::pill(ui, "Vaulted", NEGATIVE());
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!(
                                        "{} / {}",
                                        relic_service::rarity_chance_label(*rarity, Refinement::Intact),
                                        relic_service::rarity_chance_label(*rarity, Refinement::Radiant)
                                    ))
                                    .size(12.0)
                                    .color(TEXT_SECONDARY()),
                                );
                            });
                        });
                    }
                });
            }
            let ui = &mut cols[1];
            kit::card(ui, |ui| {
                ui.set_width(ui.available_width());
                kit::card_title(ui, "Drop locations");
                match &state.drops {
                    None => {
                        ui.spinner();
                    }
                    Some(drops) => {
                        let sources = drops.lookup(&item.name);
                        if sources.is_empty() {
                            ui.label(RichText::new("No drop table entries.").color(TEXT_MUTED()));
                        }
                        for s in sources.iter().take(40) {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.label(&s.location);
                                    if !s.detail.is_empty() {
                                        ui.label(RichText::new(&s.detail).size(11.5).color(TEXT_MUTED()));
                                    }
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(RichText::new(format!("{:.2}%", s.chance)).family(theme::body_strong()));
                                });
                            });
                        }
                    }
                }
            });
        });
    }
}
