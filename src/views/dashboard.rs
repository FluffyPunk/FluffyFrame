//! Dashboard (original: `src/views/DashboardView.svelte`, `src/components/widgets/*`).

use chrono::{Datelike, Duration, TimeZone, Utc};
use egui::{Align, FontId, Layout, RichText, Ui};
use fluffyframe_core::services::foundry;

use super::ViewName;
use crate::assets;
use crate::components::{format, ui as kit};
use crate::ipc::Backend;
use crate::stores::AppState;
use crate::theme::{self, *};

#[derive(Default)]
pub struct DashboardView;

/// Widget card with icon, title and an optional "open" link.
fn widget(ui: &mut Ui, icon: egui::ImageSource<'static>, title: &str, go: Option<(&mut Option<ViewName>, ViewName)>, add: impl FnOnce(&mut Ui)) {
    kit::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            kit::icon(ui, icon, 16.0);
            ui.label(RichText::new(title).font(FontId::new(16.0, theme::display_bold())));
            if let Some((target, view)) = go {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button("↗").on_hover_text(format!("Open {}", view.label())).clicked() {
                        *target = Some(view);
                    }
                });
            }
        });
        ui.add_space(4.0);
        add(ui);
    });
    ui.add_space(12.0);
}

fn row_line(ui: &mut Ui) {
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(r.x_range(), r.top(), egui::Stroke::new(1.0, BORDER_SUBTLE()));
}

impl DashboardView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, _backend: &Backend) -> Option<ViewName> {
        let mut go = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let width = ui.available_width().min(1300.0);
            let pad = (ui.available_width() - width) / 2.0;
            ui.horizontal(|ui| {
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    kit::page_title(ui, "Dashboard", None);
                    ui.add_space(8.0);
                    ui.columns(2, |cols| {
                        self.left(&mut cols[0], state, &mut go);
                        self.right(&mut cols[1], state, &mut go);
                    });
                });
            });
        });
        go
    }

    fn left(&self, ui: &mut Ui, state: &AppState, go: &mut Option<ViewName>) {
        if let Some(ws) = &state.world {
            widget(ui, assets::view_icon(ViewName::World), "Planet Cycles", Some((go, ViewName::World)), |ui| {
                for c in &ws.cycles {
                    ui.horizontal(|ui| {
                        kit::icon(ui, assets::cycle_icon(c.name), 32.0);
                        ui.label(RichText::new(c.name).family(theme::body_strong()).size(15.0));
                        ui.label(RichText::new(c.state).size(12.0).color(TEXT_SECONDARY()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(format::until(Some(c.ends_at))).family(theme::body_strong()));
                            ui.label(RichText::new(format!("{} in", next_state(c.name, c.state))).size(11.5).color(TEXT_MUTED()));
                        });
                    });
                    row_line(ui);
                    ui.add_space(4.0);
                }
            });

            widget(ui, assets::RELIC, "Void Fissures", Some((go, ViewName::World)), |ui| {
                let sp: Vec<_> = ws.fissures.iter().filter(|f| f.steel_path && !f.storm).collect();
                let list: Vec<_> = if sp.is_empty() { ws.fissures.iter().filter(|f| !f.storm).collect() } else { sp };
                kit::caption(ui, if list.first().is_some_and(|f| f.steel_path) { "Steel Path" } else { "Normal" });
                for f in list.iter().take(6) {
                    kit::split_row(
                        ui,
                        |ui| {
                            kit::pill(ui, &f.tier, theme::era_color(&f.tier));
                            ui.label(RichText::new(format!("{} · {}", f.mission_type, f.node)).color(TEXT_SECONDARY()));
                        },
                        |ui| {
                            ui.label(format::until(f.expiry));
                        },
                    );
                }
                if list.len() > 6 {
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.label(RichText::new(format!("+{} more", list.len() - 6)).size(11.0).color(TEXT_MUTED()));
                    });
                }
            });
        } else {
            widget(ui, assets::view_icon(ViewName::World), "Planet Cycles", None, |ui| {
                ui.spinner();
            });
        }

        if let (Some(inv), Some(db)) = (&state.inventory, &state.db) {
            let pending = foundry::pending(inv, db);
            widget(ui, assets::view_icon(ViewName::Foundry), "Foundry ready", Some((go, ViewName::Foundry)), |ui| {
                if pending.is_empty() {
                    kit::empty(ui, "Nothing building", "Start a build in the Foundry.");
                }
                for p in pending.iter().take(6) {
                    kit::split_row(
                        ui,
                        |ui| {
                            ui.label(RichText::new(&p.name).color(TEXT_SECONDARY()));
                        },
                        |ui| match p.completes_at {
                            Some(t) if t <= Utc::now() => {
                                kit::caption(ui, "Ready to claim");
                            }
                            t => {
                                ui.label(format::until(t));
                            }
                        },
                    );
                }
                if pending.len() > 6 {
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.label(RichText::new(format!("+{} more", pending.len() - 6)).size(11.0).color(TEXT_MUTED()));
                    });
                }
            });
        }
    }

    fn right(&self, ui: &mut Ui, state: &AppState, go: &mut Option<ViewName>) {
        if let Some(baro) = state.world.as_ref().and_then(|w| w.void_trader.clone()) {
            widget(ui, assets::DUCATS, "Baro Ki'Teer", Some((go, ViewName::World)), |ui| {
                kit::caption(ui, &baro.location);
                if baro.is_active(Utc::now()) {
                    ui.label(format!("Baro leaves in {} · {} items", format::until(baro.expiry), baro.inventory.len()));
                } else {
                    ui.label(format!("Baro arrives in {}", format::until(baro.activation)));
                }
            });
        }

        if let Some(inv) = &state.inventory {
            widget(ui, assets::PLATINUM, "Wallet", Some((go, ViewName::Stats)), |ui| {
                let c = &inv.currencies;
                let items = [
                    (assets::PLATINUM, "Platinum", format::int(c.platinum)),
                    (assets::DUCATS_SMALL, "Ducats", format::int(c.ducats)),
                    (assets::AYA, "Aya", format::int(c.aya)),
                    (assets::MASTERY, "Mastery Rank", inv.mastery_rank.to_string()),
                ];
                // 4, 2 or 1 per row: each needs about 120px before its label wraps.
                let per_row = match (ui.available_width() / 120.0) as usize {
                    0 | 1 => 1,
                    2 | 3 => 2,
                    _ => 4,
                };
                for row in items.chunks(per_row) {
                    ui.columns(per_row, |cols| {
                        for (col, (icon, label, value)) in cols.iter_mut().zip(row) {
                            currency(col, icon.clone(), label, value.clone());
                        }
                    });
                }
            });

            widget(ui, assets::view_icon(ViewName::Inventory), "Est. value", Some((go, ViewName::Inventory)), |ui| {
                kit::caption(ui, "Prime parts only");
                let (plat, ducats) = state.rows.iter().filter(|r| r.is_prime_part).fold((0.0, 0u64), |(p, d), r| {
                    (p + state.row_price(r).unwrap_or(0.0) * r.count as f64, d + r.ducats.unwrap_or(0) as u64 * r.count)
                });
                ui.columns(2, |cols| {
                    kit::caption(&mut cols[0], "Platinum");
                    cols[0].label(RichText::new(format::int(plat.round() as u64)).size(18.0));
                    kit::caption(&mut cols[1], "Ducats");
                    cols[1].label(RichText::new(format::int(ducats)).size(18.0));
                });
            });
        }

        widget(ui, assets::TRADE, "Trade summary", Some((go, ViewName::Stats)), |ui| {
            kit::caption(ui, "This month");
            let now = Utc::now();
            let month_start = Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0).single().unwrap_or(now - Duration::days(30));
            let (earned, spent) = state.trades.plat_totals(month_start);
            let count = state.trades.events.iter().filter(|e| e.at >= month_start).count();
            if count == 0 {
                kit::empty(ui, "", "No trades recorded yet");
            } else {
                ui.columns(3, |cols| {
                    kit::caption(&mut cols[0], "Earned");
                    cols[0].label(RichText::new(format!("+{earned}")).size(18.0).color(POSITIVE()));
                    kit::caption(&mut cols[1], "Spent");
                    cols[1].label(RichText::new(format!("−{spent}")).size(18.0).color(NEGATIVE()));
                    kit::caption(&mut cols[2], "Trades");
                    cols[2].label(RichText::new(count.to_string()).size(18.0));
                });
            }
        });

        if let (Some(arbi), Some(db)) = (&state.arbi, &state.db)
            && let Some(cur) = arbi.current(Utc::now())
        {
            widget(ui, assets::view_icon(ViewName::World), "Arbitration", Some((go, ViewName::World)), |ui| {
                let r = db.regions.get(&cur.node_id);
                kit::caption(ui, r.map(|r| r.faction.as_str()).unwrap_or(""));
                kit::split_row(
                    ui,
                    |ui| {
                        ui.label(RichText::new(db.node_label(&cur.node_id)).family(theme::body_strong()));
                        ui.label(RichText::new(r.map(|r| r.mission_type.as_str()).unwrap_or("")).color(TEXT_SECONDARY()));
                    },
                    |ui| {
                        ui.label(format::until(Some(cur.end())));
                    },
                );
            });
        }
    }
}

fn currency(ui: &mut Ui, icon: egui::ImageSource<'static>, label: &str, value: String) {
    ui.horizontal(|ui| {
        kit::icon(ui, icon, 22.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            kit::caption(ui, label);
            ui.label(RichText::new(value).size(17.0).family(theme::body_strong()));
        });
    });
}

fn next_state(name: &str, state: &str) -> &'static str {
    match (name, state) {
        (_, "Day") => "Night",
        (_, "Night") => "Day",
        (_, "Warm") => "Cold",
        (_, "Cold") => "Warm",
        (_, "Fass") => "Vome",
        (_, "Vome") => "Fass",
        _ => "Next",
    }
}
