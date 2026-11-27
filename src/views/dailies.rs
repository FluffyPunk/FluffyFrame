//! Dailies/Weeklies tracker (original: `src/components/world/DailiesTracker.svelte`,
//! `src/lib/world/dailies.ts`, `dailiesLive.ts`).
//!
//! Ticks persist in settings keyed by the reset they belong to, so a task clears
//! itself when its own period (daily, weekly, sortie, archon, Baro…) rolls over.

use chrono::{DateTime, Utc};
use egui::{Align, FontId, Layout, Margin, RichText, Stroke, Ui, vec2};
use fluffyframe_core::config::settings::Settings;
use fluffyframe_core::services::world_state::{self, WorldState};

use crate::components::{format, ui as kit};
use crate::ipc::Backend;
use crate::stores::{self, AppState};
use crate::theme::{self, *};
use crate::i18n::{tr, trf, trn, tkey};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    Daily,
    Weekly,
    Vendors,
}

struct Task {
    id: String,
    group: Group,
    label: String,
    detail: String,
    target: u32,
    wiki: &'static str,
    /// When this task's period ends; ticks are keyed by it.
    period: Option<DateTime<Utc>>,
}

/// Ticks done for `id` in the period ending at `period`.
pub fn progress(s: &Settings, id: &str, period: Option<DateTime<Utc>>) -> u32 {
    let Some(p) = period else { return 0 };
    match s.checklist.get(&format!("{id}@{}", p.timestamp())) {
        Some(n) => *n as u32,
        None => 0,
    }
}

fn set_progress(s: &mut Settings, id: &str, period: Option<DateTime<Utc>>, n: u32) {
    let Some(p) = period else { return };
    let key = format!("{id}@{}", p.timestamp());
    if n == 0 {
        s.checklist.remove(&key);
    } else {
        s.checklist.insert(key, n as i64);
    }
    // Drop ticks of periods that already ended.
    let now = Utc::now().timestamp();
    s.checklist.retain(|k, _| k.rsplit('@').next().and_then(|t| t.parse::<i64>().ok()).is_none_or(|t| t > now));
}

fn tasks(ws: &WorldState, state: &AppState) -> Vec<Task> {
    let now = Utc::now();
    let daily = Some(world_state::next_daily_reset(now));
    let weekly = Some(world_state::next_weekly_reset(now));
    let t = |id: &str, group, label: &str, detail: String, target, wiki, period| Task {
        id: id.into(),
        group,
        label: label.into(),
        detail,
        target,
        wiki,
        period,
    };
    let standing = state.inventory.as_ref().and_then(|i| i.daily_standing).map(|d| trf("{} standing left today", &[&(format::int(d.max(0) as u64))])).unwrap_or_default();
    let sortie = ws.sortie.as_ref();
    let archon = ws.archon_hunt.as_ref();
    let circuit = ws.circuit.as_ref();
    let baro = ws.void_trader.as_ref();
    vec![
        t("sortie", Group::Daily, tr("Sortie"), sortie.map(|s| trf("Boss: {}", &[&s.boss])).unwrap_or_default(), 1, "Sortie", sortie.and_then(|s| s.expiry)),
        t("spIncursions", Group::Daily, tr("Steel Path Incursions"), String::new(), 5, "Steel Path", daily),
        t("simaris", Group::Daily, tr("Simaris synthesis target"), String::new(), 1, "Cephalon Simaris", daily),
        t("syndicateStanding", Group::Daily, tr("Syndicate standing cap"), standing, 1, "Syndicate", daily),
        t("dailyFocus", Group::Daily, tr("Daily Focus cap"), String::new(), 1, "Focus", daily),
        t(
            "archonHunt",
            Group::Weekly,
            tr("Archon Hunt"),
            archon.map(|a| trf("Boss: {} - {} Archon Shard this week", &[&a.boss, &world_state::archon_shard(&a.boss)])).unwrap_or_default(),
            1,
            "Archon Hunt",
            archon.and_then(|a| a.expiry),
        ),
        t("circuitNormal", Group::Weekly, tr("The Circuit"), circuit.map(|c| c.normal.join(" - ")).unwrap_or_default(), 1, "The Circuit", weekly),
        t("circuitSteelPath", Group::Weekly, tr("The Circuit (Steel Path)"), circuit.map(|c| c.steel_path.join(" - ")).unwrap_or_default(), 1, "The Circuit", weekly),
        t("netracells", Group::Weekly, tr("Netracells"), String::new(), 5, "Netracell", weekly),
        t("deepArchimedea", Group::Weekly, tr("Deep Archimedea"), String::new(), 1, "Deep Archimedea", weekly),
        t("temporalArchimedea", Group::Weekly, tr("Temporal Archimedea"), String::new(), 1, "Temporal Archimedea", weekly),
        t("kahl", Group::Weekly, tr("Break Narmer (Kahl)"), String::new(), 1, "Kahl's Garrison", weekly),
        t("clem", Group::Weekly, tr("Help Clem"), String::new(), 1, "Clem", weekly),
        t("ayatanHunt", Group::Weekly, tr("Ayatan Treasure Hunt"), String::new(), 1, "Maroo", weekly),
        t(
            "steelPathHonors",
            Group::Weekly,
            tr("Steel Path Honors (Teshin)"),
            ws.steel_path.as_ref().map(|s| trf("{} - {} Steel Essence", &[&s.reward, &s.cost])).unwrap_or_default(),
            1,
            "Teshin",
            weekly,
        ),
        t("palladino", Group::Vendors, tr("Palladino (Iron Wake)"), String::new(), 1, "Palladino", weekly),
        t("acrithis", Group::Vendors, tr("Acrithis weekly stock"), String::new(), 1, "Acrithis", weekly),
        t(
            "bird3",
            Group::Vendors,
            tr("Bird 3 Archon Shard"),
            archon.map(|a| trf("{} Archon Shard this week", &[&world_state::archon_shard(&a.boss)])).unwrap_or_default(),
            1,
            "Bird 3",
            weekly,
        ),
        t("yonta", Group::Vendors, tr("Yonta weekly Kuva trade"), String::new(), 1, "Archimedean Yonta", weekly),
        t(
            "baro",
            Group::Vendors,
            tr("Visit Baro Ki'Teer"),
            baro.map(|b| if b.is_active(now) { trf("Here at {}", &[&format::node(&b.location)]) } else { trf("Next stop {}", &[&format::node(&b.location)]) }).unwrap_or_default(),
            1,
            "Baro Ki'Teer",
            baro.and_then(|b| if b.is_active(now) { b.expiry } else { b.activation }),
        ),
        t(
            "varzia",
            Group::Vendors,
            tr("Visit Varzia"),
            ws.prime_resurgence.as_ref().map(|p| trn("{} offer", "{} offers", (p.total_offers) as u64, &[&p.total_offers])).unwrap_or_default(),
            1,
            "Prime Resurgence",
            ws.prime_resurgence.as_ref().and_then(|p| p.expiry),
        ),
        t(
            "darvo",
            Group::Vendors,
            tr("Darvo's deal"),
            ws.darvo.as_ref().map(|d| format!("{} - {}p", d.name, d.sale_price)).unwrap_or_default(),
            1,
            "Darvo",
            ws.darvo.as_ref().and_then(|d| d.expiry),
        ),
    ]
}

#[derive(Default)]
pub struct DailiesView {
    search: String,
}

struct RowSpec<'a> {
    label: &'a str,
    detail: &'a str,
    done: u32,
    target: u32,
    expiry: Option<DateTime<Utc>>,
    wiki: &'a str,
    standing: Option<u32>,
}

/// A checklist row; returns the new tick count when changed.
fn task_row(ui: &mut Ui, row: RowSpec) -> Option<u32> {
    let RowSpec { label, detail, done, target, expiry, wiki, standing } = row;
    let mut changed = None;
    egui::Frame::new().inner_margin(Margin::symmetric(8, 5)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let mut checked = done >= target;
            if ui.checkbox(&mut checked, "").changed() {
                changed = Some(if checked { target } else { 0 });
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let color = if done >= target { TEXT_MUTED() } else { TEXT() };
                let mut text = RichText::new(label).color(color);
                if done >= target {
                    text = text.strikethrough();
                }
                ui.label(text);
                if !detail.is_empty() {
                    ui.label(RichText::new(detail).size(11.5).color(TEXT_MUTED()));
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !wiki.is_empty() && ui.small_button("↗").on_hover_text(trf("{} on the wiki", &[&wiki])).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://wiki.warframe.com/w/{}", wiki.replace(' ', "_"))));
                }
                if target > 1 {
                    if ui.small_button("+").clicked() && done < target {
                        changed = Some(done + 1);
                    }
                    ui.label(RichText::new(format!("{done}/{target}")).size(12.0));
                    if ui.small_button("−").clicked() && done > 0 {
                        changed = Some(done - 1);
                    }
                } else if let Some(t) = expiry {
                    ui.label(RichText::new(format::until(Some(t))).size(11.5).color(TEXT_MUTED()));
                }
                if let Some(st) = standing {
                    kit::pill(ui, &trf("{} standing", &[&format::int(st as u64)]), ACCENT());
                }
            });
        });
    });
    let r = ui.min_rect();
    ui.painter().hline(r.x_range(), ui.cursor().top() - 2.0, Stroke::new(1.0, BORDER()));
    changed
}

fn group_card(ui: &mut Ui, title: &str, done: usize, total: usize, resets: Option<DateTime<Utc>>, add: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).font(FontId::new(16.0, theme::display_bold())))
        .default_open(true)
        .show(ui, |ui| {
            kit::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{done}/{total}")).size(11.5).color(TEXT_MUTED()));
                    let w = ui.available_width() - 140.0;
                    ui.allocate_ui(vec2(w, 6.0), |ui| {
                        ui.set_width(w);
                        kit::progress(ui, done as f32 / total.max(1) as f32, POSITIVE(), 3.0);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(t) = resets {
                            ui.label(RichText::new(trf("Resets in {}", &[&(format::until(Some(t)))])).size(11.5).color(TEXT_MUTED()));
                        }
                    });
                });
                add(ui);
            });
        });
    ui.add_space(8.0);
}

impl DailiesView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend, ws: &WorldState) {
        let mut s = stores::settings(backend);
        let before = s.checklist.clone();
        let all = tasks(ws, state);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Tick off what you have done. Progress resets with each task's own timer.")).color(TEXT_SECONDARY()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                kit::search(ui, &mut self.search, tr("Search..."), 200.0);
            });
        });
        ui.add_space(6.0);
        let q = self.search.trim().to_lowercase();
        let now = Utc::now();
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.columns(2, |cols| {
                let render_group = |ui: &mut Ui, group: Group, title: &str, resets: Option<DateTime<Utc>>, s: &mut Settings| {
                    let list: Vec<&Task> = all.iter().filter(|t| t.group == group).filter(|t| q.is_empty() || t.label.to_lowercase().contains(&q)).collect();
                    let done = list.iter().filter(|t| progress(s, &t.id, t.period) >= t.target).count();
                    group_card(ui, title, done, list.len(), resets, |ui| {
                        for t in list {
                            let n = progress(s, &t.id, t.period);
                            if let Some(new) = task_row(
                                ui,
                                RowSpec {
                                    label: &t.label,
                                    detail: &t.detail,
                                    done: n,
                                    target: t.target,
                                    expiry: t.period.filter(|_| t.target == 1),
                                    wiki: t.wiki,
                                    standing: None,
                                },
                            ) {
                                set_progress(s, &t.id, t.period, new);
                            }
                        }
                    });
                };
                render_group(&mut cols[0], Group::Daily, tr("Daily"), Some(world_state::next_daily_reset(now)), &mut s);

                // Nightwave acts.
                if let Some(nw) = &ws.nightwave {
                    let ui = &mut cols[0];
                    let acts: Vec<_> = nw.challenges.iter().filter(|c| q.is_empty() || c.title.to_lowercase().contains(&q)).collect();
                    let id_of = |c: &fluffyframe_core::services::world_state::NightwaveChallenge| format!("nw:{}", c.title);
                    let done = acts.iter().filter(|c| progress(&s, &id_of(c), c.expiry) >= 1).count();
                    let header = trf("Season {} ends in {}", &[&nw.season, &format::until(nw.expiry)]);
                    group_card(ui, tr("Nightwave"), done, acts.len(), None, |ui| {
                        ui.label(RichText::new(header).size(11.5).color(TEXT_MUTED()));
                        for kind in [tkey("Daily"), tkey("Weekly"), tkey("Elite Weekly")] {
                            let in_kind: Vec<_> = acts.iter().filter(|c| c.kind == kind).collect();
                            if in_kind.is_empty() {
                                continue;
                            }
                            kit::caption(ui, &trf("{} acts", &[&tr(kind)]));
                            for c in in_kind {
                                let id = id_of(c);
                                let n = progress(&s, &id, c.expiry);
                                if let Some(new) = task_row(
                                    ui,
                                    RowSpec { label: &c.title, detail: "", done: n, target: 1, expiry: c.expiry, wiki: "", standing: Some(c.standing) },
                                ) {
                                    set_progress(&mut s, &id, c.expiry, new);
                                }
                            }
                        }
                    });
                }

                render_group(&mut cols[1], Group::Weekly, tr("Weekly"), Some(world_state::next_weekly_reset(now)), &mut s);
                render_group(&mut cols[1], Group::Vendors, tr("Vendors"), None, &mut s);
            });
        });
        if s.checklist != before {
            let checklist = s.checklist.clone();
            stores::update_settings(backend, |cur| cur.checklist = checklist);
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(1));
    }
}
