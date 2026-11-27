//! Item detail modal (original: `ItemDetailModal.svelte`, `ComponentPanel.svelte`,
//! `DropsList.svelte`, `CraftingTree.svelte`).

use egui::{Align, FontId, Layout, Margin, RichText, Stroke, Ui, vec2};
use fluffyframe_core::services::inventory::{ARCHON_SHARD_SLOTS, ArchonShard};
use fluffyframe_core::services::item_database::{ItemDb, ItemKind};

use crate::components::{archon, format, ui as kit, widgets};
use crate::ipc::Backend;
use crate::stores::AppState;
use crate::theme::{self, *};
use crate::i18n::{tr, trf};

#[derive(Default)]
pub struct ItemModal {
    /// Component shown in the side panel.
    component: Option<String>,
    crafting_tree: bool,
}

/// Relic and drop-table sources (original: `resolveDrops`). Components also list
/// their blueprint's relics, since that is what drops for Warframe parts; a
/// finished item does not, or a Prime frame would show its main blueprint's relics.
fn drops_section(ui: &mut Ui, state: &AppState, db: &ItemDb, unique: &str) {
    let name = db.name(unique);
    let mut targets = vec![unique.to_string()];
    if db.get(unique).is_some_and(|i| i.kind == ItemKind::Component)
        && let Some(bp) = db.recipe_by_result.get(unique)
    {
        targets.push(bp.clone());
    }
    let relics: Vec<_> = targets.iter().flat_map(|u| db.relics_dropping(u)).collect();
    if !relics.is_empty() {
        ui.label(RichText::new(tr("Relics")).font(FontId::new(15.0, theme::display_bold())));
        for (relic, rarity) in relics.iter().take(12) {
            ui.horizontal(|ui| {
                kit::icon(ui, crate::assets::relic_icon(relic.era.label()), 18.0);
                ui.label(&relic.name);
                kit::pill(ui, tr(rarity.label()), TEXT_SECONDARY());
                if state.relic_vaulted(&relic.family) == Some(true) {
                    kit::pill(ui, tr("Vaulted"), NEGATIVE());
                }
            });
        }
        ui.add_space(6.0);
    }
    let sources = state.drops.as_ref().map(|d| d.lookup(&name).to_vec()).unwrap_or_default();
    if !sources.is_empty() {
        ui.label(RichText::new(tr("Drops")).font(FontId::new(15.0, theme::display_bold())));
        for s in sources.iter().take(10) {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format::drop_location(&s.location)).size(13.0));
                if !s.detail.is_empty() {
                    ui.label(RichText::new(&s.detail).size(11.5).color(TEXT_MUTED()));
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{:.2}%", s.chance)).family(theme::body_strong()));
                });
            });
        }
    }
}

/// Socketed archon shards per owned copy, five sockets each with colour and
/// granted stat (original: ItemDetailModal's "Archon Shards" section).
fn shards_section(ui: &mut Ui, state: &AppState, unique: &str) {
    let Some(inv) = state.inventory.as_ref() else { return };
    // Copies with any socket entry; DE omits the array on frames never sharded.
    let copies: Vec<&[Option<ArchonShard>]> = inv
        .equipment
        .iter()
        .filter(|e| e.unique_name == unique && !e.shard_slots.is_empty())
        .map(|e| e.shard_slots.as_slice())
        .collect();
    if copies.is_empty() {
        return;
    }
    ui.label(RichText::new(tr("Archon Shards")).font(FontId::new(16.0, theme::display_bold())));
    for (ci, shards) in copies.iter().enumerate() {
        if copies.len() > 1 {
            ui.label(RichText::new(trf("Copy {}", &[&(ci + 1)])).size(11.5).family(theme::body_strong()).color(TEXT_MUTED()));
        }
        for slot in 0..ARCHON_SHARD_SLOTS.max(shards.len()) {
            let shard = shards.get(slot).and_then(Option::as_ref);
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                archon::paint_socket(ui, state, shard, rect);
                ui.allocate_ui(vec2(150.0, 18.0), |ui| {
                    ui.set_width(150.0);
                    match shard {
                        Some(s) => ui.label(RichText::new(archon::shard_name(s)).size(13.0).color(TEXT_SECONDARY())),
                        None => ui.label(RichText::new(tr("None")).size(13.0).color(TEXT_MUTED())),
                    };
                });
                if let Some(s) = shard {
                    ui.label(RichText::new(s.stat_label()).size(13.0).color(TEXT_SECONDARY()));
                }
            });
            let y = ui.cursor().top() - 2.0;
            let x = ui.min_rect().x_range();
            ui.painter().hline(x, y, Stroke::new(1.0, BORDER_SUBTLE()));
        }
    }
    ui.add_space(8.0);
}

/// Recursive crafting tree rows.
fn tree(ui: &mut Ui, db: &ItemDb, counts: &std::collections::HashMap<&str, u64>, unique: &str, need: u64, depth: usize, open: &mut Option<String>) {
    if depth > 4 {
        return;
    }
    let have = counts.get(unique).copied().unwrap_or(0);
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 22.0);
        kit::remote_image(ui, db.image_url(unique).as_deref(), vec2(24.0, 24.0));
        if ui.link(db.name(unique)).clicked() {
            *open = Some(unique.to_string());
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| kit::have_need(ui, have, need));
    });
    if have < need
        && let Some(r) = db.recipe_for_result(unique)
    {
        for ing in &r.ingredients {
            tree(ui, db, counts, &ing.item_type, ing.count as u64 * (need - have), depth + 1, open);
        }
    }
}

impl ItemModal {
    pub fn show(&mut self, ctx: &egui::Context, state: &mut AppState, _backend: &Backend) {
        let Some(unique) = state.item_stack.last().cloned() else {
            self.component = None;
            self.crafting_tree = false;
            return;
        };
        let Some(db) = state.db.clone() else { return };
        let info = db.get(&unique).cloned();
        let name = db.name(&unique);
        let counts = state.inventory.as_ref().map(|i| i.count_map()).unwrap_or_default();
        let mut open_next: Option<String> = None;
        let mut back = false;
        let mut close = false;

        let frame = egui::Frame::new()
            .fill(BG_SURFACE())
            .stroke(Stroke::new(1.0, BORDER_STRONG()))
            .corner_radius(RADIUS_LG())
            .inner_margin(Margin::same(18));
        let resp = egui::Modal::new(egui::Id::unique("item-modal")).frame(frame).show(ctx, |ui| {
            let wide = self.component.is_some() || self.crafting_tree;
            ui.set_width(if wide { 940.0 } else { 620.0 });
            // Top actions.
            ui.horizontal(|ui| {
                if state.item_stack.len() > 1 && kit::ghost_button(ui, tr("‹ Back")).clicked() {
                    back = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        close = true;
                    }
                    widgets::wiki_button(ui, &name);
                    let recipe = db.recipe_for_result(&unique).or_else(|| db.recipes.get(&unique));
                    if recipe.is_some() && kit::chip(ui, self.crafting_tree, if self.crafting_tree { tr("Back to details") } else { tr("Crafting Tree") }).clicked() {
                        self.crafting_tree = !self.crafting_tree;
                    }
                });
            });

            if self.crafting_tree {
                ui.label(RichText::new(&name).font(FontId::new(20.0, theme::display_bold())));
                kit::caption(ui, tr("Crafting tree"));
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                    let result = db.recipes.get(&unique).map(|r| r.result_type.clone()).unwrap_or(unique.clone());
                    if let Some(r) = db.recipe_for_result(&result) {
                        for ing in &r.ingredients {
                            tree(ui, &db, &counts, &ing.item_type, ing.count as u64, 0, &mut open_next);
                        }
                    }
                });
                return;
            }

            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(if wide { 600.0 } else { ui.available_width() });
                    // Header.
                    ui.horizontal_top(|ui| {
                        egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_MD()).inner_margin(Margin::same(8)).show(ui, |ui| {
                            kit::remote_image(ui, db.image_url(&unique).as_deref(), vec2(104.0, 104.0));
                        });
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&name).font(FontId::new(26.0, theme::display_bold())));
                            ui.horizontal_wrapped(|ui| {
                                if name.contains("Prime") {
                                    kit::pill(ui, tr("Prime"), ACCENT());
                                }
                                if state.rows.iter().any(|r| r.unique_name == unique && r.vaulted) {
                                    kit::pill(ui, tr("Vaulted"), NEGATIVE());
                                }
                                let parent = state.parent(&unique).cloned();
                                let target = parent.clone().unwrap_or(unique.clone());
                                if state.mastered.contains(&target) {
                                    kit::pill(ui, tr("Mastered"), POSITIVE());
                                }
                                if parent.as_ref().is_some_and(|p| counts.get(p.as_str()).copied().unwrap_or(0) > 0) {
                                    kit::pill(ui, tr("Parent owned"), INFO());
                                }
                                if let Some(i) = &info {
                                    ui.label(RichText::new(tr(i.kind.label())).size(12.0).color(TEXT_MUTED()));
                                }
                                if let Some(p) = parent
                                    && kit::ghost_button(ui, &trf("Part of {}", &[&db.name(&p)])).clicked()
                                {
                                    open_next = Some(p);
                                }
                            });
                            let owned = counts.get(unique.as_str()).copied().unwrap_or(0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(trf("Owned {}", &[&format::int(owned)])).size(12.5).color(TEXT_SECONDARY()));
                                if let Some(i) = &info
                                    && i.mastery_req > 0
                                {
                                    ui.label(RichText::new(trf("MR {}", &[&i.mastery_req])).size(12.5).color(TEXT_SECONDARY()));
                                }
                            });
                            if let Some(i) = &info
                                && !i.description.is_empty()
                            {
                                ui.label(RichText::new(&i.description).size(12.5).color(TEXT_SECONDARY()));
                            }
                        });
                    });
                    ui.add_space(8.0);

                    // Components.
                    let recipe = db.recipe_for_result(&unique).or_else(|| db.recipes.get(&unique));
                    if let Some(r) = recipe {
                        ui.label(RichText::new(tr("Components")).font(FontId::new(16.0, theme::display_bold())));
                        let mut rows: Vec<(String, u64)> = vec![(r.unique_name.clone(), 1)];
                        if db.recipes.contains_key(&unique) {
                            rows.clear();
                        }
                        rows.extend(r.ingredients.iter().map(|i| (i.item_type.clone(), i.count as u64)));
                        // "Gara Prime Chassis" reads "Chassis" under Gara Prime, sorted by that label.
                        let prefix = format!("{name} ");
                        let mut rows: Vec<(String, u64, String)> = rows
                            .into_iter()
                            .map(|(comp, need)| {
                                let full = db.name(&comp);
                                let label = full.strip_prefix(&prefix).map(str::to_string).unwrap_or(full);
                                (comp, need, label)
                            })
                            .collect();
                        rows.sort_by(|a, b| a.2.cmp(&b.2));
                        for (comp, need, label) in rows {
                            let have = counts.get(comp.as_str()).copied().unwrap_or(0);
                            let bp_held = db.recipe_by_result.get(&comp).is_some_and(|b| counts.get(b.as_str()).copied().unwrap_or(0) > 0);
                            let color = if have >= need { POSITIVE() } else if have > 0 || bp_held { WARNING() } else { NEGATIVE() };
                            let selected = self.component.as_deref() == Some(comp.as_str());
                            let resp = egui::Frame::new()
                                .fill(if selected { alpha(ACCENT(), 0.12) } else { egui::Color32::TRANSPARENT })
                                .corner_radius(RADIUS_SM())
                                .inner_margin(Margin::symmetric(4, 5))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(label).size(15.0));
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.label(RichText::new(format!("{}/{}", have, need)).color(color).family(theme::body_strong()))
                                                .on_hover_text(if bp_held { tr("Blueprint owned, not built") } else { "" });
                                        });
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());
                            if resp.clicked() {
                                self.component = if selected { None } else { Some(comp.clone()) };
                            }
                            let r = ui.min_rect();
                            ui.painter().hline(r.x_range(), ui.cursor().top() - 3.0, Stroke::new(1.0, BORDER_SUBTLE()));
                        }
                        ui.add_space(8.0);
                    }

                    shards_section(ui, state, &unique);

                    if !wide {
                        drops_section(ui, state, &db, &unique);
                    }

                    ui.label(RichText::new(tr("Warframe.market")).font(FontId::new(16.0, theme::display_bold())));
                    match state.catalog.as_ref().and_then(|c| c.by_game_ref(&unique)) {
                        Some(c) => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(tr("48h average")).color(TEXT_MUTED()));
                                kit::plat(ui, state.price_of_unique(&unique), 14.0);
                                if let Some(d) = info.as_ref().and_then(|i| i.ducats) {
                                    kit::ducats(ui, Some(d), 14.0);
                                }
                                widgets::open_link(ui, tr("Open WFM"), &c.market_url());
                            });
                        }
                        None => {
                            ui.label(RichText::new(tr("Item is not tradable.")).size(12.5).color(TEXT_MUTED()));
                        }
                    }
                });

                // Component side panel.
                if let Some(comp) = self.component.clone() {
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(280.0);
                        ui.horizontal(|ui| {
                            kit::remote_image(ui, db.image_url(&comp).as_deref(), vec2(40.0, 40.0));
                            ui.label(RichText::new(db.name(&comp)).font(FontId::new(17.0, theme::display_bold())));
                        });
                        ui.horizontal(|ui| {
                            kit::plat(ui, state.price_of_unique(&comp), 13.0);
                            if kit::ghost_button(ui, tr("Open")).clicked() {
                                open_next = Some(comp.clone());
                            }
                        });
                        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                            drops_section(ui, state, &db, &comp);
                        });
                    });
                }
            });
            let _ = ItemKind::Other;
        });

        if resp.should_close() || close {
            state.item_stack.clear();
        } else if back {
            state.item_stack.pop();
            self.component = None;
        } else if let Some(next) = open_next {
            self.component = None;
            self.crafting_tree = false;
            state.open_item(next);
        }
    }
}
