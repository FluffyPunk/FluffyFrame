//! Stats (original: `src/views/StatsView.svelte`, `src/components/stats/*`).

use chrono::{Duration, NaiveDate, Utc};
use egui::{Color32, FontId, ImageSource, Margin, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};
use wfhelper_core::services::ee_log_monitor::{Direction, TradeKind};
use wfhelper_core::services::stats_tracker::DayStats;

use crate::assets;
use crate::components::{format, ui as kit};
use crate::stores::AppState;
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum TradeFilter {
    #[default]
    All,
    Sale,
    Purchase,
    Trade,
}

pub struct StatsView {
    days: i64,
    trade_filter: TradeFilter,
    trade_search: String,
}

impl Default for StatsView {
    fn default() -> Self {
        Self { days: 30, trade_filter: TradeFilter::All, trade_search: String::new() }
    }
}

struct Metric {
    label: &'static str,
    icon: Option<ImageSource<'static>>,
    /// DE unique name whose export icon to show when there is no bundled one.
    remote: Option<&'static str>,
    get: fn(&DayStats) -> u64,
}

fn metrics() -> [Metric; 7] {
    [
        Metric { label: "Platinum", icon: Some(assets::PLATINUM), remote: None, get: |d| d.platinum },
        Metric { label: "Ducats", icon: Some(assets::DUCATS_SMALL), remote: None, get: |d| d.ducats },
        Metric { label: "Aya", icon: Some(assets::AYA), remote: None, get: |d| d.aya },
        Metric { label: "Credits", icon: None, remote: None, get: |d| d.credits },
        Metric { label: "Endo", icon: None, remote: None, get: |d| d.endo },
        Metric { label: "Vitus Essence", icon: Some(assets::VITUS), remote: None, get: |d| d.vitus },
        Metric { label: "Kuva", icon: None, remote: Some("/Lotus/Types/Items/MiscItems/Kuva"), get: |d| d.kuva },
    ]
}

fn metric_icon(ui: &mut Ui, state: &AppState, m: &Metric, size: f32) {
    if let Some(icon) = &m.icon {
        kit::icon(ui, icon.clone(), size);
    } else if let Some(url) = m.remote.and_then(|u| state.db.as_ref()?.image_url(u)) {
        kit::remote_image(ui, Some(&url), vec2(size, size));
    }
}

/// Axis tick text: "6.0K", "-2.0K", "20.0M", "35".
fn axis_label(v: f64) -> String {
    let a = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if a >= 1e6 {
        format!("{sign}{:.1}M", a / 1e6)
    } else if a >= 1e3 {
        format!("{sign}{:.1}K", a / 1e3)
    } else {
        format!("{sign}{a:.0}")
    }
}

/// Line of values plus green/red bars for day-over-day change.
fn chart(ui: &mut Ui, points: &[(String, u64, i64)], height: f32) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let painter = ui.painter_at(rect);
    let font = FontId::new(10.5, egui::FontFamily::Proportional);
    if points.len() < 2 {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Not enough days recorded yet", FontId::proportional(12.0), TEXT_MUTED());
        return;
    }
    let inner = egui::Rect::from_min_max(rect.min + vec2(44.0, 6.0), rect.max - vec2(8.0, 18.0));
    let values: Vec<f64> = points.iter().map(|p| p.1 as f64).collect();
    let deltas: Vec<f64> = points.iter().map(|p| p.2 as f64).collect();
    let lo = values.iter().chain(&deltas).cloned().fold(f64::MAX, f64::min).min(0.0);
    let hi = values.iter().chain(&deltas).cloned().fold(f64::MIN, f64::max).max(1.0);
    let span = (hi - lo).max(1.0);
    let y = |v: f64| inner.bottom() - inner.height() * ((v - lo) / span) as f32;
    let x = |i: usize| inner.left() + inner.width() * i as f32 / (points.len() - 1) as f32;

    for k in 0..=4 {
        let v = lo + span * k as f64 / 4.0;
        let yy = y(v);
        painter.hline(inner.x_range(), yy, Stroke::new(1.0, alpha(Color32::WHITE, 0.06)));
        painter.text(pos2(inner.left() - 6.0, yy), egui::Align2::RIGHT_CENTER, axis_label(v), font.clone(), TEXT_MUTED());
    }
    let zero = y(0.0);
    let bar_w = (inner.width() / points.len() as f32 * 0.5).clamp(2.0, 14.0);
    for (i, d) in deltas.iter().enumerate() {
        if *d == 0.0 {
            continue;
        }
        let top = y(*d);
        let r = egui::Rect::from_x_y_ranges(x(i) - bar_w / 2.0..=x(i) + bar_w / 2.0, top.min(zero)..=top.max(zero));
        painter.rect_filled(r, 1.0, if *d > 0.0 { alpha(POSITIVE(), 0.8) } else { alpha(NEGATIVE(), 0.8) });
    }
    let line: Vec<_> = values.iter().enumerate().map(|(i, v)| pos2(x(i), y(*v))).collect();
    painter.add(egui::Shape::line(line.clone(), Stroke::new(1.5, Color32::from_gray(230))));
    let step = (points.len() / 6).max(1);
    for (i, (day, _, _)) in points.iter().enumerate().step_by(step) {
        let label = NaiveDate::parse_from_str(day, "%Y-%m-%d").map(|d| d.format("%-m/%-d").to_string()).unwrap_or_default();
        painter.text(pos2(x(i), inner.bottom() + 4.0), egui::Align2::CENTER_TOP, label, font.clone(), TEXT_MUTED());
    }
    if let Some(h) = resp.hover_pos() {
        let i = (((h.x - inner.left()) / inner.width()) * (points.len() - 1) as f32).round().clamp(0.0, (points.len() - 1) as f32) as usize;
        painter.circle(line[i], 4.0, BG_SURFACE(), Stroke::new(1.5, Color32::WHITE));
        let (day, v, d) = &points[i];
        painter.text(line[i] + vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, format!("{day}: {} ({d:+})", format::int(*v)), FontId::proportional(12.0), TEXT());
    }
}

impl StatsView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState) {
        kit::page_title(ui, "Stats", None);
        kit::tabs(ui, &mut 0, &[(0, "Tracking".into())]);

        egui::Panel::right("trade-history")
            .resizable(false)
            .exact_size(300.0)
            .frame(egui::Frame::new().inner_margin(Margin { left: 16, right: 0, top: 0, bottom: 0 }))
            .show(ui, |ui| self.trades(ui, state));

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::ComboBox::from_id_salt("timeframe").width(70.0).selected_text(format!("{}d", self.days)).show_ui(ui, |ui| {
                    for d in [7, 30, 90, 365] {
                        ui.selectable_value(&mut self.days, d, format!("{d}d"));
                    }
                });
                ui.label(RichText::new("Timeframe:").size(12.0).color(TEXT_MUTED()));
            });
        });

        let cutoff = (chrono::Local::now() - Duration::days(self.days)).format("%Y-%m-%d").to_string();
        let today = state.stats.days.values().last().copied().unwrap_or_default();
        let yesterday = state.stats.days.values().rev().nth(1).copied().unwrap_or(today);
        let ms = metrics();

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            kit::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    for m in &ms {
                        let v = (m.get)(&today);
                        let d = v as i64 - (m.get)(&yesterday) as i64;
                        ui.horizontal(|ui| {
                            metric_icon(ui, state, m, 26.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(m.label.to_uppercase()).font(FontId::new(14.0, theme::display_bold())));
                                    ui.label(RichText::new(format::compact(v)).font(FontId::new(17.0, theme::display_bold())));
                                });
                                ui.label(RichText::new(format!("{}{} today", if d >= 0 { "+" } else { "−" }, format::compact(d.unsigned_abs()))).size(11.0).color(TEXT_MUTED()));
                            });
                        });
                        ui.add_space(14.0);
                    }
                });
            });
            ui.add_space(10.0);
            ui.columns(2, |cols| {
                for (i, m) in ms.iter().enumerate() {
                    let ui = &mut cols[i % 2];
                    let series: Vec<_> = state.stats.series(m.get).into_iter().filter(|(d, _, _)| *d >= cutoff).collect();
                    kit::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            metric_icon(ui, state, m, 16.0);
                            ui.label(m.label);
                        });
                        chart(ui, &series, 180.0);
                    });
                    ui.add_space(10.0);
                }
            });
        });
    }

    fn trades(&mut self, ui: &mut Ui, state: &AppState) {
        kit::caption(ui, "Trade history");
        let count = |k: Option<TradeKind>| state.trades.events.iter().filter(|e| k.is_none_or(|k| e.trade.kind == k)).count();
        egui::Grid::new("trade-filters").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
            for (i, (f, label, k)) in [
                (TradeFilter::All, "All", None),
                (TradeFilter::Sale, "Sale", Some(TradeKind::Sale)),
                (TradeFilter::Purchase, "Purchase", Some(TradeKind::Purchase)),
                (TradeFilter::Trade, "Trade", Some(TradeKind::Trade)),
            ]
            .into_iter()
            .enumerate()
            {
                let selected = self.trade_filter == f;
                let btn = egui::Button::new(RichText::new(format!("{label}  {}", count(k))).color(if selected { TEXT_ON_ACCENT() } else { TEXT_SECONDARY() }))
                    .fill(if selected { ACCENT() } else { BG_SURFACE() })
                    .min_size(vec2(135.0, 26.0));
                if ui.add(btn).clicked() {
                    self.trade_filter = f;
                }
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
        ui.add_space(4.0);
        kit::search(ui, &mut self.trade_search, "Search items or players...", ui.available_width());
        let q = self.trade_search.trim().to_lowercase();
        let list: Vec<_> = state
            .trades
            .events
            .iter()
            .rev()
            .filter(|t| match self.trade_filter {
                TradeFilter::All => true,
                TradeFilter::Sale => t.trade.kind == TradeKind::Sale,
                TradeFilter::Purchase => t.trade.kind == TradeKind::Purchase,
                TradeFilter::Trade => t.trade.kind == TradeKind::Trade,
            })
            .filter(|t| q.is_empty() || t.trade.partner.to_lowercase().contains(&q) || t.trade.items.iter().any(|i| i.name.to_lowercase().contains(&q)))
            .collect();
        if list.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No trades recorded yet").family(theme::body_strong()));
                ui.label(
                    RichText::new("Trades are detected automatically from your game log when you accept a trade in-game. Keep the app running while playing to begin tracking.")
                        .size(12.0)
                        .color(TEXT_MUTED()),
                );
            });
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for t in list.iter().take(300) {
                kit::raised(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (label, color) = match t.trade.kind {
                            TradeKind::Sale => ("Sale", POSITIVE()),
                            TradeKind::Purchase => ("Purchase", NEGATIVE()),
                            TradeKind::Trade => ("Trade", INFO()),
                        };
                        kit::pill(ui, label, color);
                        ui.label(RichText::new(&t.trade.partner).family(theme::body_strong()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            kit::plat(ui, Some(t.trade.platinum as f64), 12.0);
                        });
                    });
                    for i in &t.trade.items {
                        let arrow = if i.direction == Direction::Given { "→" } else { "←" };
                        ui.label(RichText::new(format!("{arrow} {} ×{}", i.name, i.count)).size(12.0).color(TEXT_SECONDARY()));
                    }
                    ui.label(RichText::new(format::local_datetime(t.at)).size(11.0).color(TEXT_MUTED()));
                });
                ui.add_space(4.0);
            }
        });
        let _ = Utc::now();
    }
}
