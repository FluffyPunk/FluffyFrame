//! World (original: `src/views/WorldView.svelte`, `src/components/world/*`,
//! `src/lib/world/baroPlanner.ts`).

use chrono::Utc;
use egui::{Align, FontId, Layout, Margin, RichText, Ui, vec2};
use egui_extras::{Column, TableBuilder};
use fluffyframe_core::services::world_state::WorldState;

use super::arbi::ArbiView;
use super::need_game_data;
use crate::assets;
use crate::components::{format, ui as kit};
use crate::ipc::{Backend, DataCmd};
use crate::stores::AppState;
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    World,
    Arbitrations,
    Dailies,
    Baro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum FissureFilter {
    #[default]
    Normal,
    SteelPath,
    Storms,
}

#[derive(Default)]
pub struct WorldView {
    tab: Tab,
    fissures: FissureFilter,
    tier: Option<&'static str>,
    baro_search: String,
    baro_unowned: bool,
    arbi: ArbiView,
    dailies_view: super::dailies::DailiesView,
}


impl WorldView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        if need_game_data(ui, state) {
            return;
        }
        ui.horizontal(|ui| {
            kit::page_title(ui, "World", None);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(b) = state.world.as_ref().and_then(|w| w.void_trader.clone()) {
                    let text = if b.is_active(Utc::now()) {
                        format!("Baro leaves in {} - {}", format::until(b.expiry), b.location)
                    } else {
                        format!("Baro arrives in {} - {}", format::until(b.activation), b.location)
                    };
                    kit::chip(ui, true, &text);
                }
                if kit::ghost_button(ui, "⟳ Refresh").clicked() {
                    backend.data(DataCmd::RefreshWorld);
                }
                if let Some(t) = state.world.as_ref().and_then(|w| w.fetched_at) {
                    ui.label(RichText::new(format!("Updated {}", format::ago(t))).size(12.0).color(TEXT_MUTED()));
                }
            });
        });
        kit::tabs(
            ui,
            &mut self.tab,
            &[
                (Tab::World, "World".into()),
                (Tab::Arbitrations, "Arbitrations".into()),
                (Tab::Dailies, "Dailies/Weeklies".into()),
                (Tab::Baro, "Baro".into()),
            ],
        );
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
        if self.tab == Tab::Arbitrations {
            self.arbi.ui(ui, state, backend);
            return;
        }
        let Some(ws) = state.world.clone() else {
            ui.spinner();
            return;
        };
        match self.tab {
            Tab::World => self.world(ui, state, backend, &ws),
            Tab::Dailies => self.dailies_view.ui(ui, state, backend, &ws),
            Tab::Baro => self.baro(ui, state, backend, &ws),
            Tab::Arbitrations => {}
        }
    }

    fn world(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend, ws: &WorldState) {
        let settings = crate::stores::settings(backend);
        let counter = |id: &str, period: Option<chrono::DateTime<Utc>>| super::dailies::progress(&settings, id, period);
        let mut open: Option<String> = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            // This week.
            ui.horizontal(|ui| {
                ui.label(RichText::new("This week").font(FontId::new(18.0, theme::display_bold())));
                ui.label(RichText::new(format!("Daily reset: {}   Weekly resets: {}", format::until(ws.daily_reset), format::until(ws.weekly_reset))).size(11.5).color(TEXT_MUTED()));
            });
            ui.add_space(4.0);
            let week = ws.weekly_reset;
            let mut cards: Vec<WeekCard> = Vec::new();
            if let Some(a) = &ws.archon_hunt {
                let shard = fluffyframe_core::services::world_state::archon_shard(&a.boss);
                cards.push(("Archon Hunt", counter("archonHunt", a.expiry), 1, format!("Boss: {} - {shard} Archon Shard this week", a.boss), a.expiry));
            }
            if let Some(c) = &ws.circuit {
                cards.push(("The Circuit (Steel Path)", counter("circuitSteelPath", week), 1, c.steel_path.join(" - "), c.expiry));
            }
            cards.push(("Netracells", counter("netracells", week), 5, String::new(), week));
            if let Some(sp) = &ws.steel_path {
                cards.push(("Steel Path Honors (Teshin)", counter("steelPathHonors", week), 1, format!("{} - {} Steel Essence", sp.reward, sp.cost), Some(sp.expiry)));
            }
            let per_row = kit::balanced(kit::fit_columns(ui, 220.0, cards.len()), cards.len());
            for row in cards.chunks(per_row) {
                ui.columns(per_row, |cols| {
                    for (col, (title, done, target, detail, resets)) in cols.iter_mut().zip(row) {
                        week_card(col, title, *done, *target, detail, *resets);
                    }
                });
            }
            ui.add_space(10.0);

            // Two columns, stacked on narrow windows.
            kit::responsive_columns(ui, 420.0, 2, |ui, col| {
                if col == 0 {
                section(ui, "Planet Cycles", |ui| {
                    let per_row = kit::fit_columns(ui, 260.0, 2);
                    ui.columns(per_row, |c| {
                        for (n, cy) in ws.cycles.iter().enumerate() {
                            let ui = &mut c[n % per_row];
                            kit::split_row(
                                ui,
                                |ui| {
                                    kit::icon(ui, assets::cycle_icon(cy.name), 30.0);
                                    ui.label(RichText::new(cy.name).family(theme::body_strong()));
                                    kit::pill(ui, cy.state, cycle_color(cy.state));
                                },
                                |ui| {
                                    ui.label(RichText::new(format::until(Some(cy.ends_at))).family(theme::body_strong()));
                                },
                            );
                            ui.add_space(4.0);
                        }
                    });
                });
                section(ui, "Reset Timers", |ui| {
                    let rows = [
                        ("Daily sortie", ws.sortie.as_ref().and_then(|s| s.expiry)),
                        ("Daily reset", ws.daily_reset),
                        ("Weekly resets", ws.weekly_reset),
                        ("Steel Path Honors", ws.steel_path.as_ref().map(|s| s.expiry)),
                    ];
                    for (l, t) in rows {
                        kit::split_row(
                            ui,
                            |ui| {
                                ui.label(RichText::new(l).color(TEXT_SECONDARY()));
                            },
                            |ui| {
                                ui.label(format::until(t));
                            },
                        );
                    }
                });
                if let Some(pr) = &ws.prime_resurgence {
                    section(ui, "Prime Resurgence", |ui| {
                        ui.label(RichText::new(format!("Rotation ends in {} · {} offers", format::until(pr.expiry), pr.total_offers)).size(12.0).color(TEXT_SECONDARY()));
                        ui.horizontal_wrapped(|ui| {
                            for it in &pr.featured {
                                let have = state.owned(&it.unique_name) > 0 || state.mastered.contains(&it.unique_name);
                                if image_tile(ui, state, &it.unique_name, &it.name, have).on_hover_text(format!("{} Aya", it.aya)).clicked() {
                                    open = Some(it.unique_name.clone());
                                }
                            }
                        });
                    });
                }
                if let Some(c) = &ws.circuit {
                    section(ui, "The Circuit", |ui| {
                        kit::caption(ui, "Normal rotation");
                        ui.horizontal_wrapped(|ui| {
                            for name in &c.normal {
                                ui.label(RichText::new(name).family(theme::body_strong()));
                            }
                        });
                        kit::caption(ui, "Steel Path rotation");
                        ui.horizontal_wrapped(|ui| {
                            for name in &c.steel_path {
                                ui.label(RichText::new(name).family(theme::body_strong()));
                            }
                        });
                    });
                }
                section(ui, "Void Fissures", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (f, l) in [(FissureFilter::Normal, "Normal"), (FissureFilter::SteelPath, "Steel Path"), (FissureFilter::Storms, "Void Storms")] {
                            if kit::chip(ui, self.fissures == f, l).clicked() {
                                self.fissures = f;
                            }
                        }
                        ui.separator();
                        if kit::chip(ui, self.tier.is_none(), "All").clicked() {
                            self.tier = None;
                        }
                        for t in ["Lith", "Meso", "Neo", "Axi", "Requiem", "Omnia"] {
                            if kit::chip(ui, self.tier == Some(t), t).clicked() {
                                self.tier = Some(t);
                            }
                        }
                    });
                    for f in ws.fissures.iter().filter(|f| match self.fissures {
                        FissureFilter::Normal => !f.storm && !f.steel_path,
                        FissureFilter::SteelPath => !f.storm && f.steel_path,
                        FissureFilter::Storms => f.storm,
                    }) {
                        if self.tier.is_some_and(|t| t != f.tier) {
                            continue;
                        }
                        kit::split_row(
                            ui,
                            |ui| {
                                kit::icon(ui, assets::relic_icon(&f.tier), 20.0);
                                kit::pill(ui, &f.tier, theme::era_color(&f.tier));
                                ui.label(RichText::new(&f.mission_type).family(theme::body_strong()));
                                ui.label(RichText::new(&f.node).color(TEXT_SECONDARY()));
                            },
                            |ui| {
                                ui.label(format::until(f.expiry));
                            },
                        );
                    }
                });

                } else {
                section(ui, "Invasions", |ui| {
                    for inv in &ws.invasions {
                        ui.label(RichText::new(&inv.node).font(FontId::new(16.0, theme::display_bold())));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(inv.attacker.to_uppercase()).family(theme::body_strong()).color(theme::faction_color(&inv.attacker)));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(inv.defender.to_uppercase()).family(theme::body_strong()).color(theme::faction_color(&inv.defender)));
                                let rewards = match (inv.attacker_reward.is_empty(), inv.defender_reward.is_empty()) {
                                    (false, false) => format!("{}  VS  {}", inv.attacker_reward, inv.defender_reward),
                                    (true, false) => format!("VS  {}", inv.defender_reward),
                                    (false, true) => format!("{}  VS", inv.attacker_reward),
                                    (true, true) => "VS".into(),
                                };
                                ui.label(RichText::new(rewards).color(ACCENT()));
                            });
                        });
                        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 3.0), egui::Sense::hover());
                        let split = rect.left() + rect.width() * inv.progress as f32;
                        ui.painter().rect_filled(egui::Rect::from_x_y_ranges(rect.left()..=split, rect.y_range()), 1.0, theme::faction_color(&inv.attacker));
                        ui.painter().rect_filled(egui::Rect::from_x_y_ranges(split..=rect.right(), rect.y_range()), 1.0, theme::faction_color(&inv.defender));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{:.1}%", inv.progress * 100.0)).size(11.5).color(theme::faction_color(&inv.attacker)));
                            ui.label(RichText::new(format!("{:.1}%", (1.0 - inv.progress) * 100.0)).size(11.5).color(theme::faction_color(&inv.defender)));
                        });
                        ui.add_space(6.0);
                    }
                });
                if let Some(d) = &ws.darvo {
                    section(ui, "Darvo's Deal", |ui| {
                        ui.horizontal(|ui| {
                            egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_MD()).inner_margin(Margin::same(4)).show(ui, |ui| {
                                kit::remote_image(ui, state.db.as_ref().and_then(|db| db.image_url(&d.unique_name)).as_deref(), vec2(52.0, 52.0));
                            });
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&d.name).family(theme::body_strong()).size(15.0));
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("{}p", d.sale_price)).family(theme::body_strong()).color(ACCENT_BRIGHT()));
                                    ui.label(RichText::new(format!("{}p", d.original_price)).strikethrough().color(TEXT_MUTED()));
                                    kit::pill(ui, &format!("-{}%", d.discount), POSITIVE());
                                });
                                ui.label(RichText::new(format!("{}/{} sold · ends in {}", d.sold, d.total, format::until(d.expiry))).size(12.0).color(TEXT_MUTED()));
                            });
                        });
                    });
                }
                if !ws.alerts.is_empty() {
                    section(ui, "Alerts", |ui| {
                        for a in &ws.alerts {
                            kit::split_row(
                                ui,
                                |ui| {
                                    ui.label(RichText::new(&a.mission_type).family(theme::body_strong()));
                                    ui.label(RichText::new(&a.node).color(TEXT_SECONDARY()));
                                },
                                |ui| {
                                    ui.label(format::until(a.expiry));
                                },
                            );
                            ui.label(RichText::new(&a.reward).size(12.0).color(ACCENT()));
                        }
                    });
                }
                section(ui, "Bounties", |ui| {
                    for b in &ws.bounties {
                        egui::CollapsingHeader::new(RichText::new(format!("{}  ·  {}", b.syndicate, format::until(b.expiry))).family(theme::body_strong()))
                            .id_salt(&b.syndicate)
                            .show(ui, |ui| {
                                for j in &b.jobs {
                                    ui.label(RichText::new(j).size(12.5).color(TEXT_SECONDARY()));
                                }
                            });
                    }
                });
                }
            });
        });
        if let Some(u) = open {
            state.open_item(u);
        }
    }

    fn baro(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend, ws: &WorldState) {
        let Some(baro) = &ws.void_trader else {
            kit::empty(ui, "No Baro data", "The world state lists no Void Trader.");
            return;
        };
        let active = baro.is_active(Utc::now());
        ui.label(RichText::new("Baro planner").font(FontId::new(22.0, theme::display())));
        ui.label(RichText::new(if active {
            format!("Baro is at {} and leaves in {}.", baro.location, format::until(baro.expiry))
        } else {
            format!("Baro is away. Arrives at {} in {}.", baro.location, format::until(baro.activation))
        })
        .color(TEXT_SECONDARY()));
        ui.add_space(8.0);
        if let Some(inv) = &state.inventory {
            kit::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.columns(2, |cols| {
                    kit::raised(&mut cols[0], |ui| {
                        ui.set_width(ui.available_width());
                        kit::caption(ui, "Ducats");
                        ui.label(RichText::new(format::int(inv.currencies.ducats)).size(22.0));
                    });
                    kit::raised(&mut cols[1], |ui| {
                        ui.set_width(ui.available_width());
                        kit::caption(ui, "Credits");
                        ui.label(RichText::new(format::int(inv.currencies.credits)).size(22.0));
                    });
                });
            });
            ui.add_space(8.0);
        }
        if !active {
            kit::empty(ui, "Inventory appears when Baro arrives", "His stock is only listed while he is at a relay.");
            return;
        }
        ui.horizontal(|ui| {
            kit::search(ui, &mut self.baro_search, "Search...", 240.0);
            ui.checkbox(&mut self.baro_unowned, "Unowned only");
        });
        let counts = state.inventory.as_ref().map(|i| i.count_map()).unwrap_or_default();
        let q = self.baro_search.trim().to_lowercase();
        let items: Vec<_> = baro
            .inventory
            .iter()
            .filter(|i| q.is_empty() || i.name.to_lowercase().contains(&q))
            .filter(|i| !self.baro_unowned || counts.get(i.unique_name.as_str()).copied().unwrap_or(0) == 0)
            .collect();
        state.request_prices_for(backend, items.iter().map(|i| i.unique_name.clone()));
        let db = state.db.clone();
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::exact(52.0))
            .column(Column::remainder().at_least(220.0))
            .column(Column::exact(90.0))
            .column(Column::exact(110.0))
            .column(Column::exact(90.0))
            .header(26.0, |mut h| {
                for t in ["", "Item", "Ducats", "Credits", "Market"] {
                    h.col(|ui| {
                        kit::caption(ui, t);
                    });
                }
            })
            .body(|body| {
                body.rows(52.0, items.len(), |mut row| {
                    let it = items[row.index()];
                    let owned = counts.get(it.unique_name.as_str()).copied().unwrap_or(0);
                    row.col(|ui| {
                        kit::remote_image(ui, db.as_ref().and_then(|d| d.image_url(&it.unique_name)).as_deref(), vec2(44.0, 44.0));
                    });
                    row.col(|ui| {
                        ui.vertical(|ui| {
                            ui.label(&it.name);
                            if owned > 0 {
                                ui.label(RichText::new("Owned").size(11.5).color(POSITIVE()));
                            }
                        });
                    });
                    row.col(|ui| {
                        ui.label(it.ducats.map(|d| d.to_string()).unwrap_or_default());
                    });
                    row.col(|ui| {
                        ui.label(it.credits.map(format::int).unwrap_or_default());
                    });
                    row.col(|ui| {
                        if let Some(p) = state.price_of_unique(&it.unique_name) {
                            kit::plat(ui, Some(p), 13.0);
                        }
                    });
                });
            });
    }
}

/// Collapsible titled block (original: `CollapsibleSection.svelte`).
fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).font(FontId::new(16.0, theme::display_bold())))
        .default_open(true)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
    ui.add_space(8.0);
}

/// (title, done, target, detail, resets) for [`week_card`].
type WeekCard<'a> = (&'a str, u32, u32, String, Option<chrono::DateTime<Utc>>);

fn week_card(ui: &mut Ui, title: &str, done: u32, target: u32, detail: &str, resets: Option<chrono::DateTime<Utc>>) {
    kit::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(72.0);
        kit::split_row(
            ui,
            |ui| {
                ui.label(RichText::new(title).family(theme::body_strong()));
            },
            |ui| {
                let color = if done >= target { POSITIVE() } else { TEXT_MUTED() };
                ui.label(RichText::new(format!("{done}/{target}")).size(12.0).color(color));
            },
        );
        if !detail.is_empty() {
            ui.label(RichText::new(detail).size(12.0).color(TEXT_SECONDARY()));
        }
        ui.label(RichText::new(format!("Resets in {}", format::until(resets))).size(11.5).color(TEXT_MUTED()));
    });
}

fn image_tile(ui: &mut Ui, state: &AppState, unique: &str, name: &str, highlight: bool) -> egui::Response {
    let resp = ui
        .vertical(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(96.0, 96.0), egui::Sense::hover());
            let stroke = if highlight { egui::Stroke::new(2.0, POSITIVE()) } else { egui::Stroke::new(1.0, BORDER_STRONG()) };
            ui.painter().rect(rect, RADIUS_MD(), BG_RAISED(), stroke, egui::StrokeKind::Inside);
            if let Some(url) = state.db.as_ref().and_then(|db| db.image_url(unique)) {
                kit::paint_image_fit(ui, &url, rect.shrink(6.0));
            }
            ui.allocate_ui(vec2(96.0, 16.0), |ui| {
                ui.set_width(96.0);
                ui.vertical_centered(|ui| ui.add(egui::Label::new(RichText::new(name).size(11.5).color(TEXT_SECONDARY())).truncate()));
            });
        })
        .response;
    resp.interact(egui::Sense::click())
}

fn cycle_color(state: &str) -> egui::Color32 {
    match state {
        "Day" | "Fass" => WARNING(),
        "Warm" => egui::Color32::from_rgb(0xf9, 0x73, 0x16),
        "Night" | "Cold" | "Vome" => INFO(),
        _ => TEXT_SECONDARY(),
    }
}
