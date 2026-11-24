//! Inventory (original: `src/views/InventoryView.svelte`, `src/components/inventory/*`,
//! `ItemDetailModal.svelte`, `BulkSellModal.svelte`, `src/lib/inventory/valueTotals.ts`).

use std::collections::HashSet;

use egui::{Align, FontId, Layout, Margin, RichText, Sense, Stroke, Ui, pos2, vec2};
use wfhelper_core::config::settings::ValueScope;
use wfhelper_core::services::full_sets;
use wfhelper_core::services::inventory::{InventoryGroup, InventoryRow};
use wfhelper_core::services::wfm_catalog::Catalog;
use wfhelper_core::services::wfm_orders::{NewOrder, OrderType};
use wfhelper_core::services::wfm_stats_price::price_key;

use super::need_inventory;
use crate::components::{archon, format, ui as kit};
use crate::ipc::{Backend, MarketCmd, PriceCmd};
use crate::stores::{self, AppState, price_rank};
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Sort {
    #[default]
    Name,
    Count,
    Price,
    Total,
    Ducats,
    DucatsPerPlat,
}

impl Sort {
    const ALL: [Sort; 6] = [Sort::Name, Sort::Count, Sort::Price, Sort::Total, Sort::Ducats, Sort::DucatsPerPlat];
    fn label(self) -> &'static str {
        match self {
            Sort::Name => "Name",
            Sort::Count => "Count",
            Sort::Price => "Price",
            Sort::Total => "Total value",
            Sort::Ducats => "Ducats",
            Sort::DucatsPerPlat => "Ducats / plat",
        }
    }
}

struct SellDraft {
    order_type: OrderType,
    item_id: String,
    name: String,
    rank: Option<u32>,
    platinum: u32,
    quantity: u32,
}

struct BulkLine {
    item_id: String,
    name: String,
    rank: Option<u32>,
    platinum: u32,
    quantity: u32,
    include: bool,
}

pub struct InventoryView {
    tab: InventoryGroup,
    search: String,
    sort: Sort,
    descending: bool,
    online_only: bool,
    /// Item to open in the detail modal on the next frame.
    pending_open: Option<String>,
    rank_view: Option<u32>,
    sort_by_reputation: (bool, bool),
    filters_open: bool,
    vaulted_only: bool,
    tradable_only: bool,
    include: HashSet<InventoryGroup>,
    show_incomplete_sets: bool,
    selected: Option<String>,
    bulk_mode: bool,
    bulk_selected: HashSet<String>,
    bulk_dialog: Option<Vec<BulkLine>>,
    sell: Option<SellDraft>,
}

impl Default for InventoryView {
    fn default() -> Self {
        Self {
            tab: InventoryGroup::Everything,
            search: String::new(),
            sort: Sort::Name,
            descending: false,
            online_only: true,
            pending_open: None,
            rank_view: None,
            sort_by_reputation: (false, false),
            filters_open: false,
            vaulted_only: false,
            tradable_only: false,
            include: [
                InventoryGroup::Parts,
                InventoryGroup::Relics,
                InventoryGroup::Mods,
                InventoryGroup::Arcanes,
                InventoryGroup::Equipment,
                InventoryGroup::Resources,
                InventoryGroup::Misc,
            ]
            .into(),
            show_incomplete_sets: false,
            selected: None,
            bulk_mode: false,
            bulk_selected: HashSet::new(),
            bulk_dialog: None,
            sell: None,
        }
    }
}

fn row_key(r: &InventoryRow) -> String {
    format!("{}#{:?}", r.unique_name, r.rank)
}

fn ducats_per_plat(r: &InventoryRow, price: Option<f64>) -> Option<f64> {
    match (r.ducats, price) {
        (Some(d), Some(p)) if p > 0.0 => Some(d as f64 / p),
        _ => None,
    }
}

const LIST_ROW_H: f32 = 42.0;
const LIST_PAD: f32 = 6.0;

/// Column widths of the Rows layout: icon, name (rest), owned, price, total, ducats.
fn list_widths(width: f32, spacing: f32) -> [f32; 6] {
    let fixed = [40.0, 70.0, 80.0, 90.0, 80.0];
    let name = (width - fixed.iter().sum::<f32>() - spacing * 5.0).max(160.0);
    [fixed[0], name, fixed[1], fixed[2], fixed[3], fixed[4]]
}

fn list_cell(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(vec2(width, ui.available_height()), Layout::left_to_right(Align::Center), |ui| {
        ui.set_width(width);
        add(ui);
    });
}

/// Header of the Rows layout, aligned with [`InventoryView::list_row`] inside
/// `kit::grid_sections` (which reserves 14px for the scrollbar).
fn list_header(ui: &mut Ui) {
    let width = ui.available_width() - 14.0 - LIST_PAD * 2.0;
    let widths = list_widths(width, ui.spacing().item_spacing.x);
    ui.horizontal(|ui| {
        ui.add_space(LIST_PAD - ui.spacing().item_spacing.x);
        for (t, w) in ["", "Item", "Owned", "Price", "Total", "Ducats"].into_iter().zip(widths) {
            list_cell(ui, w, |ui| {
                kit::caption(ui, t);
            });
        }
    });
}

/// Height of [`InventoryView::item_card`] for its content, so grid rows size
/// to their tallest card like the original's CSS grid: equipment (no market
/// boxes, no rank bar) is shorter than a ranked mod. Measured from the layout:
/// image well + name/type/metric strip, then the optional blocks.
fn card_height(r: &InventoryRow) -> f32 {
    let mut h = 221.0;
    if r.wfm_slug.is_some() {
        h += 46.0;
    }
    if r.rank.is_some() && r.max_rank.is_some_and(|m| m > 0) {
        h += 32.0;
    }
    h
}

/// Compact resource tile: icon, name, amount (original: `ResourcesView.svelte`).
fn resource_tile(ui: &mut Ui, r: &InventoryRow) -> bool {
    kit::grid_card(ui, None, |ui| {
        let rect = ui.max_rect();
        let painter = ui.painter();
        let well = egui::Rect::from_center_size(pos2(rect.left() + 34.0, rect.center().y), vec2(48.0, 48.0));
        painter.rect_filled(well, RADIUS_MD(), BG_WELL());
        if let Some(url) = &r.image_url {
            kit::paint_image_fit(ui, url, well.shrink(4.0));
        }
        let left = well.right() + 10.0;
        let mut job = egui::text::LayoutJob::simple(r.name.clone(), FontId::new(14.0, theme::body_strong()), TEXT(), rect.right() - 8.0 - left);
        job.wrap.max_rows = 2;
        let name = painter.layout_job(job);
        let count = painter.layout_no_wrap(if r.count < 10_000 { format::int(r.count) } else { kit::compact_num(r.count) }, FontId::proportional(12.0), TEXT_MUTED());
        let top = rect.center().y - (name.size().y + 3.0 + count.size().y) / 2.0;
        let count_top = top + name.size().y + 3.0;
        painter.galley(pos2(left, top), name, TEXT());
        painter.galley(pos2(left, count_top), count, TEXT_MUTED());
    })
    .clicked()
}

impl InventoryView {
    /// Market items in view. Resources get their own tile section ([`Self::resources`]).
    fn filtered<'a>(&self, state: &'a AppState) -> Vec<&'a InventoryRow> {
        let q = self.search.trim().to_lowercase();
        let mut rows: Vec<&InventoryRow> = state
            .rows_in(self.tab)
            .filter(|r| r.group != InventoryGroup::Resources)
            .filter(|r| self.tab != InventoryGroup::Everything || self.include.contains(&r.group))
            .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q))
            .filter(|r| !self.vaulted_only || r.vaulted)
            .filter(|r| !self.tradable_only || r.wfm_slug.is_some())
            .collect();
        let price = |r: &InventoryRow| state.row_price(r).unwrap_or(-1.0);
        rows.sort_by(|a, b| {
            let ord = match self.sort {
                Sort::Name => a.name.cmp(&b.name),
                Sort::Count => a.count.cmp(&b.count),
                Sort::Price => price(a).total_cmp(&price(b)),
                Sort::Total => (price(a) * a.count as f64).total_cmp(&(price(b) * b.count as f64)),
                Sort::Ducats => a.ducats.cmp(&b.ducats),
                Sort::DucatsPerPlat => ducats_per_plat(a, state.row_price(a))
                    .unwrap_or(-1.0)
                    .total_cmp(&ducats_per_plat(b, state.row_price(b)).unwrap_or(-1.0)),
            };
            if self.descending { ord.reverse() } else { ord }
        });
        rows
    }

    /// Resources in view: search applies, sorted by count or else by name
    /// (original: `filterAndSortResources`).
    fn resources<'a>(&self, state: &'a AppState) -> Vec<&'a InventoryRow> {
        let shown = self.tab == InventoryGroup::Resources
            || (self.tab == InventoryGroup::Everything && self.include.contains(&InventoryGroup::Resources));
        if !shown {
            return Vec::new();
        }
        let q = self.search.trim().to_lowercase();
        let mut list: Vec<&InventoryRow> = state
            .rows_in(InventoryGroup::Resources)
            .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q))
            .collect();
        list.sort_by(|a, b| {
            let ord = if self.sort == Sort::Count { a.count.cmp(&b.count) } else { a.name.cmp(&b.name) };
            if self.descending { ord.reverse() } else { ord }
        });
        list
    }

    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        if need_inventory(ui, state) {
            return;
        }
        if let Some(u) = self.pending_open.take() {
            state.open_item(u);
        }
        let settings = stores::settings(backend);
        let total = state.rows.len();
        ui.horizontal(|ui| {
            kit::page_title(ui, "Inventory", Some(total));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let rows_mode = settings.inventory_rows;
                if kit::ghost_button(ui, if rows_mode { "▦  Cards" } else { "☰  Rows" }).clicked() {
                    stores::update_settings(backend, |s| s.inventory_rows = !rows_mode);
                }
            });
        });

        let tabs: Vec<(InventoryGroup, String)> = InventoryGroup::TABS
            .iter()
            .filter(|t| **t != InventoryGroup::Resources || state.rows_in(InventoryGroup::Resources).next().is_some())
            .map(|t| (*t, t.label().to_string()))
            .collect();
        if kit::tabs(ui, &mut self.tab, &tabs) {
            self.selected = None;
        }

        if self.tab == InventoryGroup::Sets {
            self.sets_ui(ui, state);
            return;
        }

        // Resources have no market: no price sorts, bulk sell, filters or value row.
        let market_tab = self.tab != InventoryGroup::Resources;
        if !market_tab {
            self.bulk_mode = false;
            if !matches!(self.sort, Sort::Name | Sort::Count) {
                self.sort = Sort::Name;
            }
        }
        let sorts: &[Sort] = if market_tab { &Sort::ALL } else { &[Sort::Name, Sort::Count] };

        // Toolbar: search, sort, direction, bulk sell, filters.
        ui.horizontal(|ui| {
            kit::search(ui, &mut self.search, "Search...", 240.0);
            kit::labelled(ui, "Sort", |ui| {
                egui::ComboBox::from_id_salt("inv-sort").width(150.0).selected_text(self.sort.label()).show_ui(ui, |ui| {
                    for s in sorts {
                        ui.selectable_value(&mut self.sort, *s, s.label());
                    }
                });
            });
            if kit::chip(ui, true, if self.descending { "⬇" } else { "⬆" }).on_hover_text("Sort direction").clicked() {
                self.descending = !self.descending;
            }
            if market_tab {
                if kit::chip(ui, self.bulk_mode, "Bulk Sell").clicked() {
                    self.bulk_mode = !self.bulk_mode;
                    self.bulk_selected.clear();
                }
                if kit::chip(ui, self.filters_open, "⚙ Filters").clicked() {
                    self.filters_open = !self.filters_open;
                }
            }
        });
        if market_tab && self.filters_open {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.vaulted_only, "Vaulted only");
                ui.checkbox(&mut self.tradable_only, "Tradable only");
            });
        }

        let rows = self.filtered(state);
        let resources = self.resources(state);
        if market_tab {
            self.value_row(ui, state, backend, &rows, settings.value_scope, settings.value_min_plat);
        }

        if self.tab == InventoryGroup::Everything {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Include").size(11.5).color(TEXT_MUTED()));
                for g in InventoryGroup::TABS.iter().filter(|g| !matches!(g, InventoryGroup::Everything | InventoryGroup::Sets)) {
                    let on = self.include.contains(g);
                    if kit::chip(ui, on, g.label()).clicked() {
                        if on {
                            self.include.remove(g);
                        } else {
                            self.include.insert(*g);
                        }
                    }
                }
            });
            ui.add_space(6.0);
        }

        if self.bulk_mode {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} selected", self.bulk_selected.len())).color(ACCENT()));
                if kit::ghost_button(ui, "Select all tradable").clicked() {
                    self.bulk_selected.extend(rows.iter().filter(|r| r.wfm_slug.is_some()).map(|r| row_key(r)));
                }
                if kit::ghost_button(ui, "Clear").clicked() {
                    self.bulk_selected.clear();
                }
                if !self.bulk_selected.is_empty() && kit::primary_button(ui, "  Review listings  ").clicked() {
                    self.bulk_dialog = Some(self.bulk_lines(state));
                }
            });
            ui.add_space(4.0);
        }

        if market_tab && rows.is_empty() {
            kit::empty(ui, "No items found", "Clear the search and filters, or check another category.");
        } else if !market_tab && resources.is_empty() {
            kit::empty(ui, "No resources found", "Clear the search to see every resource.");
        }

        // Market listings dock on the right while an item is selected.
        if self.selected.is_some() && !self.bulk_mode {
            egui::Panel::right("market-listings")
                .resizable(false)
                .exact_size(360.0)
                .frame(egui::Frame::new().inner_margin(Margin { left: 12, right: 0, top: 0, bottom: 0 }))
                .show(ui, |ui| self.market_panel(ui, state, backend, settings.keep_spare_copies));
        }

        // Items (cards or rows), then resources as compact tiles, in one scroll
        // (original: Everything appends `ResourcesView` under the grid).
        let mut clicked: Option<usize> = None;
        let mut clicked_resource: Option<usize> = None;
        let mut sections = Vec::new();
        if settings.inventory_rows {
            if !rows.is_empty() {
                list_header(ui);
            }
            sections.push(
                kit::GridSection::new(f32::MAX, LIST_ROW_H, rows.len(), |ui, i| {
                    if self.list_row(ui, state, rows[i], i) {
                        clicked = Some(i);
                    }
                })
                .gap(0.0),
            );
        } else {
            sections.push(
                kit::GridSection::new(200.0, 0.0, rows.len(), |ui, i| {
                    if self.item_card(ui, state, rows[i]) {
                        clicked = Some(i);
                    }
                })
                .heights(|i| card_height(rows[i])),
            );
        }
        let mut resource_section = kit::GridSection::new(208.0, 66.0, resources.len(), |ui, i| {
            if resource_tile(ui, resources[i]) {
                clicked_resource = Some(i);
            }
        });
        if market_tab {
            resource_section = resource_section.title("Resources");
        }
        sections.push(resource_section);
        kit::grid_sections(ui, sections);

        if let Some(i) = clicked_resource {
            self.pending_open = Some(resources[i].unique_name.clone());
        }
        if let Some(i) = clicked {
            let r = rows[i];
            let key = row_key(r);
            if self.bulk_mode && r.wfm_slug.is_some() {
                if !self.bulk_selected.remove(&key) {
                    self.bulk_selected.insert(key);
                }
            } else {
                if r.wfm_slug.is_none() {
                    // Nothing to trade: show the item details instead.
                    self.pending_open = Some(r.unique_name.clone());
                }
                self.selected = Some(key);
                self.rank_view = None;
                if let Some(slug) = &r.wfm_slug {
                    let rank = price_rank(r.rank, r.max_rank);
                    backend.prices(PriceCmd::Request(vec![(slug.clone(), rank)]));
                    backend.market(MarketCmd::TopOrders { slug: slug.clone(), rank });
                }
            }
        }

        self.sell_dialog(ui, state, backend);
        self.bulk_dialog_ui(ui, state, backend);
    }

    /// One grid card; returns true when clicked.
    fn item_card(&self, ui: &mut Ui, state: &AppState, r: &InventoryRow) -> bool {
        let key = row_key(r);
        let bulk_on = self.bulk_mode && self.bulk_selected.contains(&key);
        let mastered = r.level.is_some_and(|l| l >= 30);
        let highlight = if bulk_on { Some(ACCENT()) } else if mastered { Some(alpha(POSITIVE(), 0.45)) } else { None };
        let resp = kit::grid_card(ui, highlight, |ui| {
            let width = ui.available_width();
            // Image well.
            let (well, _) = ui.allocate_exact_size(vec2(width, 128.0), Sense::hover());
            ui.painter().rect_filled(
                well,
                egui::CornerRadius { nw: RADIUS_LG(), ne: RADIUS_LG(), sw: 0, se: 0 },
                BG_WELL(),
            );
            if let Some(url) = &r.image_url {
                kit::paint_image_fit(ui, url, egui::Rect::from_center_size(well.center(), vec2(96.0, 96.0)));
            }
            let count_text = format!("x{}", format::int(r.count));
            ui.painter().text(
                well.right_bottom() + vec2(-8.0, -6.0),
                egui::Align2::RIGHT_BOTTOM,
                count_text,
                FontId::new(14.0, theme::body_strong()),
                POSITIVE(),
            );
            if r.vaulted || mastered {
                let (letter, color) = if r.vaulted { ("V", NEGATIVE()) } else { ("M", POSITIVE()) };
                let c = well.left_top() + vec2(16.0, 16.0);
                ui.painter().circle(c, 8.0, alpha(color, 0.15), Stroke::new(1.0, color));
                ui.painter().text(c, egui::Align2::CENTER_CENTER, letter, FontId::new(9.5, theme::body_strong()), color);
            }
            if bulk_on {
                ui.painter().text(well.right_top() + vec2(-10.0, 8.0), egui::Align2::RIGHT_TOP, "✔", FontId::proportional(14.0), ACCENT());
            }
            archon::shard_pips(ui, state, &r.shards, well.left_bottom() + vec2(8.0, -8.0));

            egui::Frame::new().inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(&r.name).family(theme::body_strong()).size(14.0)).truncate());
                let mut sub = r.kind.label().to_string();
                if let Some(level) = r.level {
                    sub = format!("{sub} · Lv {level}");
                }
                ui.label(RichText::new(sub).size(12.0).color(TEXT_MUTED()));
                let price = state.row_price(r);
                ui.horizontal(|ui| {
                    kit::plat(ui, price, 13.0);
                    ui.add_space(6.0);
                    kit::ducats(ui, r.ducats, 13.0);
                    ui.add_space(6.0);
                    ui.label(RichText::new("d/p").size(11.0).color(TEXT_MUTED()));
                    let dpp = ducats_per_plat(r, price).map(|v| format!("{v:.1}")).unwrap_or("-".into());
                    ui.label(RichText::new(dpp).family(theme::body_strong()).color(ACCENT()));
                });
                if let Some(slug) = &r.wfm_slug {
                    ui.add_space(2.0);
                    let rank = price_rank(r.rank, r.max_rank);
                    let book = state.top_orders.get(&price_key(slug, rank));
                    let rank_label = rank.map(|r| format!(" R{r}")).unwrap_or_default();
                    let (wts, wtb) = match book {
                        Some(b) => (
                            b.sell.first().map(|o| format!("{}p", o.platinum)),
                            b.buy.first().map(|o| format!("{}p", o.platinum)),
                        ),
                        None => (price.map(|p| format!("~{p:.0}p")), None),
                    };
                    ui.columns(2, |cols| {
                        price_box(&mut cols[0], &format!("WTS{rank_label}"), wts.as_deref());
                        price_box(&mut cols[1], &format!("WTB{rank_label}"), wtb.as_deref());
                    });
                }
                if let (Some(rank), Some(max)) = (r.rank, r.max_rank.filter(|m| *m > 0)) {
                    ui.add_space(4.0);
                    let color = if rank >= max { POSITIVE() } else { ACCENT() };
                    kit::progress(ui, rank as f32 / max as f32, color, 3.0);
                    ui.label(RichText::new(format!("{rank}/{max}")).size(11.0).color(TEXT_MUTED()));
                }
            });
        });
        resp.clicked()
    }

    /// One row of the Rows layout; returns true when clicked.
    fn list_row(&self, ui: &mut Ui, state: &AppState, r: &InventoryRow, i: usize) -> bool {
        let rect = ui.max_rect();
        let resp = ui.interact(rect, ui.next_auto_id(), Sense::click());
        let fill = if self.bulk_selected.contains(&row_key(r)) {
            alpha(ACCENT(), 0.15)
        } else if resp.hovered() {
            BG_HOVER()
        } else if i % 2 == 1 {
            ui.visuals().faint_bg_color
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 0, fill);
        let inner = rect.shrink2(vec2(LIST_PAD, 0.0));
        let widths = list_widths(inner.width(), ui.spacing().item_spacing.x);
        ui.scope_builder(egui::UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)), |ui| {
            let price = state.row_price(r);
            list_cell(ui, widths[0], |ui| {
                kit::remote_image(ui, r.image_url.as_deref(), vec2(34.0, 34.0));
            });
            list_cell(ui, widths[1], |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add(egui::Label::new(RichText::new(&r.name).family(theme::body_strong())).truncate());
                    ui.label(RichText::new(r.kind.label()).size(11.5).color(TEXT_MUTED()));
                });
            });
            list_cell(ui, widths[2], |ui| {
                ui.label(format::int(r.count));
            });
            list_cell(ui, widths[3], |ui| {
                if r.wfm_slug.is_some() {
                    kit::plat(ui, price, 13.0);
                }
            });
            list_cell(ui, widths[4], |ui| {
                if let Some(p) = price {
                    kit::plat(ui, Some(p * r.count as f64), 13.0);
                }
            });
            list_cell(ui, widths[5], |ui| {
                if r.ducats.is_some() {
                    kit::ducats(ui, r.ducats, 13.0);
                }
            });
        });
        resp.clicked()
    }

    fn value_row(&self, ui: &mut Ui, state: &AppState, backend: &Backend, rows: &[&InventoryRow], scope: ValueScope, min_plat: u32) {
        let total = |rows: &mut dyn Iterator<Item = &InventoryRow>| -> (f64, u64, usize) {
            let (mut plat, mut ducats, mut unpriced) = (0.0, 0u64, 0usize);
            for r in rows {
                if r.group == InventoryGroup::Equipment || r.wfm_slug.is_none() {
                    continue;
                }
                if scope == ValueScope::PrimePartsOnly && !r.is_prime_part {
                    continue;
                }
                if r.is_prime_part {
                    ducats += r.ducats.unwrap_or(0) as u64 * r.count;
                }
                match state.row_price(r) {
                    Some(p) if p >= min_plat as f64 => plat += p * r.count as f64,
                    Some(_) => {}
                    None => unpriced += 1,
                }
            }
            (plat, ducats, unpriced)
        };
        let (plat, ducats, unpriced) = total(&mut rows.iter().copied());
        ui.horizontal(|ui| {
            kit::caption(ui, "Est. value");
            ui.add_space(8.0);
            ui.label(RichText::new("In view").size(12.0).color(TEXT_SECONDARY()));
            kit::icon(ui, crate::assets::PLATINUM, 13.0);
            let prefix = if unpriced > 0 { ">= " } else { "" };
            ui.label(RichText::new(format!("{prefix}{}", format::int(plat.round() as u64))).family(theme::body_strong()).color(ACCENT_BRIGHT()));
            kit::icon(ui, crate::assets::DUCATS_SMALL, 13.0);
            ui.label(RichText::new(format!("{prefix}{}", format::int(ducats))).family(theme::body_strong()).color(ACCENT_BRIGHT()));
            if unpriced > 0 {
                ui.label(RichText::new(format!("{unpriced} unpriced")).size(11.5).color(TEXT_MUTED()));
            }
            ui.add_space(12.0);
            if kit::chip(ui, scope == ValueScope::PrimePartsOnly, "Prime parts only").clicked() {
                stores::update_settings(backend, |s| s.value_scope = ValueScope::PrimePartsOnly);
            }
            if kit::chip(ui, scope == ValueScope::AllTradables, "All tradables").clicked() {
                stores::update_settings(backend, |s| s.value_scope = ValueScope::AllTradables);
            }
            ui.add_space(8.0);
            kit::icon(ui, crate::assets::PLATINUM, 13.0);
            for (label, v) in [("Off", 0), ("5+", 5), ("10+", 10), ("15+", 15)] {
                if kit::chip(ui, min_plat == v, label).on_hover_text("Ignore items worth less than this").clicked() {
                    stores::update_settings(backend, |s| s.value_min_plat = v);
                }
            }
        });
        ui.add_space(8.0);
    }

    /// Right-side order book (original: `InventoryOrderBookPanel.svelte`).
    fn market_panel(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend, keep_spare: u32) {
        let Some(key) = self.selected.clone() else { return };
        let Some(row) = state.rows.iter().find(|r| row_key(r) == key).cloned() else {
            self.selected = None;
            return;
        };
        kit::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Market Listings").font(FontId::new(18.0, theme::display())));
            let slug = row.wfm_slug.clone();
            let default_rank = price_rank(row.rank, row.max_rank);
            let rank = self.rank_view.or(default_rank);
            ui.horizontal(|ui| {
                if let Some(slug) = &slug {
                    if kit::ghost_button(ui, "Refresh").clicked() {
                        backend.prices(PriceCmd::Refresh(vec![(slug.clone(), rank)]));
                        backend.market(MarketCmd::TopOrders { slug: slug.clone(), rank });
                    }
                    if kit::ghost_button(ui, "Statistics").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://warframe.market/items/{slug}/statistics")));
                    }
                    if kit::ghost_button(ui, "Open WFM").clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!("https://warframe.market/items/{slug}")));
                    }
                }
                if kit::ghost_button(ui, "Details").clicked() {
                    self.pending_open = Some(row.unique_name.clone());
                }
                if kit::ghost_button(ui, "×").clicked() {
                    self.selected = None;
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_SM()).inner_margin(Margin::same(3)).show(ui, |ui| {
                    kit::remote_image(ui, row.image_url.as_deref(), vec2(46.0, 46.0));
                });
                ui.vertical(|ui| {
                    ui.label(RichText::new(&row.name).family(theme::body_strong()).size(15.0));
                    let view = rank.map(|r| format!(" · Viewing R{r}")).unwrap_or_default();
                    ui.label(RichText::new(format!("x{} · {}{view}", row.count, row.kind.label())).size(12.0).color(TEXT_MUTED()));
                });
            });

            // Safe to sell (original: inventory safety rules, simplified).
            if let (Some(inv), Some(db)) = (&state.inventory, &state.db) {
                let needed: u64 = inv
                    .items
                    .iter()
                    .filter(|i| i.collection == "Recipes")
                    .filter_map(|i| db.recipes.get(&i.unique_name).map(|r| (r, i.count)))
                    .flat_map(|(r, n)| r.ingredients.iter().filter(|g| g.item_type == row.unique_name).map(move |g| g.count as u64 * n))
                    .sum();
                let reserved = needed + keep_spare as u64;
                let safe = row.count.saturating_sub(reserved);
                ui.horizontal(|ui| {
                    kit::caption(ui, "Safe to sell");
                    ui.label(RichText::new(safe.to_string()).family(theme::body_strong()).color(if safe > 0 { POSITIVE() } else { WARNING() }));
                    if reserved > 0 {
                        ui.label(RichText::new(format!("{reserved} reserved")).size(11.5).color(TEXT_MUTED()))
                            .on_hover_text(format!("Owned blueprints need {needed}; {keep_spare} spare copies kept"));
                    }
                });
            }

            let Some(slug) = slug else {
                ui.label(RichText::new("Not tradable on warframe.market.").color(TEXT_MUTED()));
                return;
            };
            let book = state.top_orders.get(&price_key(&slug, rank)).cloned();
            let online = |o: &&wfhelper_core::services::wfm_orders::Order| !self.online_only || o.user_status.as_deref().is_some_and(|s| s != "offline");
            let mut sells: Vec<_> = book.as_ref().map(|b| b.sell.iter().filter(online).cloned().collect()).unwrap_or_default();
            let mut buys: Vec<_> = book.as_ref().map(|b| b.buy.iter().filter(online).cloned().collect()).unwrap_or_default();
            if self.sort_by_reputation.0 {
                sells.sort_by_key(|o| std::cmp::Reverse(o.reputation.unwrap_or(0)));
            }
            if self.sort_by_reputation.1 {
                buys.sort_by_key(|o| std::cmp::Reverse(o.reputation.unwrap_or(0)));
            }
            let best_wts = book.as_ref().and_then(|b| b.sell.iter().filter(online).map(|o| o.platinum).min());
            let best_wtb = book.as_ref().and_then(|b| b.buy.iter().filter(online).map(|o| o.platinum).max());
            ui.add_space(6.0);
            ui.columns(3, |cols| {
                let tile = |ui: &mut Ui, label: &str, v: Option<String>, color: egui::Color32| {
                    kit::raised(ui, |ui| {
                        ui.set_width(ui.available_width());
                        kit::caption(ui, label);
                        ui.label(RichText::new(v.unwrap_or("-".into())).family(theme::body_strong()).color(color));
                    });
                };
                tile(&mut cols[0], "Best WTS", best_wts.map(|p| format!("{p}p")), ACCENT_BRIGHT());
                tile(&mut cols[1], "Best WTB", best_wtb.map(|p| format!("{p}p")), INFO());
                let spread = best_wts.zip(best_wtb).map(|(s, b)| format!("{}p", s as i64 - b as i64));
                tile(&mut cols[2], "Spread", spread, TEXT());
            });
            ui.add_space(4.0);
            ui.checkbox(&mut self.online_only, "Online/In-game only");
            if let Some(max) = row.max_rank.filter(|m| *m > 0) {
                ui.horizontal(|ui| {
                    kit::caption(ui, "Rank view");
                    let mut r = rank.unwrap_or(0);
                    if kit::segmented(ui, &mut r, &[(0, "R0"), (max, &format!("R{max}"))]) {
                        self.rank_view = Some(r);
                        backend.market(MarketCmd::TopOrders { slug: slug.clone(), rank: Some(r) });
                        backend.prices(PriceCmd::Request(vec![(slug.clone(), Some(r))]));
                    }
                });
            }
            ui.horizontal(|ui| {
                kit::caption(ui, "WTS sort");
                kit::segmented(ui, &mut self.sort_by_reputation.0, &[(false, "Best price"), (true, "Reputation")]);
            });
            ui.horizontal(|ui| {
                kit::caption(ui, "WTB sort");
                kit::segmented(ui, &mut self.sort_by_reputation.1, &[(false, "Best offer"), (true, "Reputation")]);
            });
            ui.add_space(4.0);
            ui.columns(2, |cols| {
                for (i, (label, color, kind)) in [("Post WTS", POSITIVE(), OrderType::Sell), ("Post WTB", NEGATIVE(), OrderType::Buy)].into_iter().enumerate() {
                    let btn = egui::Button::new(RichText::new(label).color(color))
                        .fill(alpha(color, 0.12))
                        .stroke(Stroke::new(1.0, color))
                        .min_size(vec2(cols[i].available_width(), 30.0));
                    if cols[i].add(btn).clicked()
                        && let Some(item) = state.catalog.as_ref().and_then(|c| c.by_slug(&slug))
                    {
                        let suggested = match kind {
                            OrderType::Sell => best_wts.or_else(|| state.row_price(&row).map(|p| p.round() as u32)),
                            OrderType::Buy => best_wtb.or_else(|| state.row_price(&row).map(|p| (p * 0.8).round() as u32)),
                        };
                        self.sell = Some(SellDraft {
                            order_type: kind,
                            item_id: item.id.clone(),
                            name: row.name.clone(),
                            rank,
                            platinum: suggested.unwrap_or(1).max(1),
                            quantity: 1,
                        });
                    }
                }
            });
            ui.add_space(6.0);
            if book.is_none() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Loading order book…").color(TEXT_MUTED()));
                });
                return;
            }
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                ui.columns(2, |cols| {
                    for (ui, (title, list, total, color)) in cols.iter_mut().zip([
                        ("WTS", &sells, book.as_ref().map(|b| b.sell.len()).unwrap_or(0), POSITIVE()),
                        ("WTB", &buys, book.as_ref().map(|b| b.buy.len()).unwrap_or(0), NEGATIVE()),
                    ]) {
                        egui::Frame::new().fill(alpha(color, 0.10)).stroke(Stroke::new(1.0, alpha(color, 0.4))).corner_radius(RADIUS_SM()).inner_margin(Margin::same(4)).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.vertical_centered(|ui| ui.label(RichText::new(title).family(theme::body_strong()).color(color)));
                        });
                        if list.is_empty() {
                            let hidden = total.saturating_sub(list.len());
                            ui.label(RichText::new(if title == "WTS" { "No sell orders" } else { "No buy orders" }).size(12.0).color(TEXT_MUTED()));
                            if hidden > 0 {
                                ui.label(RichText::new(format!("{hidden} offline, hidden by the filter")).size(11.5).color(TEXT_MUTED()));
                            }
                        }
                        for o in list.iter().take(15) {
                            egui::Frame::new().stroke(Stroke::new(1.0, BORDER())).corner_radius(RADIUS_SM()).inner_margin(Margin::same(6)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.add(egui::Label::new(RichText::new(o.user_name.as_deref().unwrap_or("?")).size(12.5)).truncate());
                                });
                                ui.horizontal(|ui| {
                                    let (st, sc) = match o.user_status.as_deref() {
                                        Some("ingame") => ("In game", POSITIVE()),
                                        Some("online") => ("Online", INFO()),
                                        _ => ("Offline", TEXT_MUTED()),
                                    };
                                    kit::pill(ui, st, sc);
                                    ui.label(RichText::new(format!("x{}", o.quantity)).size(11.5).color(TEXT_MUTED()));
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(RichText::new(format!("{}p", o.platinum)).family(theme::body_strong()).color(ACCENT_BRIGHT()));
                                    });
                                });
                                if let Some(r) = o.rank {
                                    ui.label(RichText::new(format!("R{r}")).size(11.0).color(TEXT_MUTED()));
                                }
                                ui.columns(2, |c| {
                                    if let Some(w) = o.whisper(&row.name)
                                        && c[0].add_sized([c[0].available_width(), 22.0], egui::Button::new(RichText::new("Whisper").size(12.0))).on_hover_text(&w).clicked()
                                    {
                                        c[0].ctx().copy_text(w);
                                    }
                                    if let Some(url) = o.profile_url()
                                        && c[1].add_sized([c[1].available_width(), 22.0], egui::Button::new(RichText::new("Profile").size(12.0))).clicked()
                                    {
                                        c[1].ctx().open_url(egui::OpenUrl::new_tab(url));
                                    }
                                });
                            });
                            ui.add_space(3.0);
                        }
                    }
                });
            });
        });
    }

    fn sell_dialog(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend) {
        let Some(draft) = &mut self.sell else { return };
        let mut submit = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("sell")).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            let verb = if draft.order_type == OrderType::Sell { "Sell" } else { "Buy" };
            ui.label(RichText::new(format!("{verb} {}", draft.name)).font(FontId::new(22.0, theme::display_bold())));
            if state.session.is_none() {
                ui.label("Sign in to warframe.market in the Market view first.");
                cancel = ui.button("Close").clicked();
                return;
            }
            egui::Grid::new("sell-grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Price");
                ui.add(egui::DragValue::new(&mut draft.platinum).range(1..=100_000).suffix(" p"));
                ui.end_row();
                ui.label("Quantity");
                ui.add(egui::DragValue::new(&mut draft.quantity).range(1..=999));
                ui.end_row();
                if let Some(r) = draft.rank {
                    ui.label("Rank");
                    ui.label(r.to_string());
                    ui.end_row();
                }
            });
            ui.label(RichText::new("The listing is created when you confirm. Finish the trade in game.").size(12.0).color(TEXT_MUTED()));
            ui.horizontal(|ui| {
                submit = kit::primary_button(ui, "  Create listing  ").clicked();
                cancel = kit::ghost_button(ui, "Cancel").clicked();
            });
        });
        if submit {
            backend.create_orders(vec![NewOrder {
                item_id: draft.item_id.clone(),
                order_type: draft.order_type,
                platinum: draft.platinum,
                quantity: draft.quantity,
                rank: draft.rank,
                visible: true,
            }]);
        }
        if submit || cancel {
            self.sell = None;
        }
    }

    fn bulk_lines(&self, state: &AppState) -> Vec<BulkLine> {
        let catalog = state.catalog.as_deref();
        state
            .rows
            .iter()
            .filter(|r| self.bulk_selected.contains(&row_key(r)))
            .filter_map(|r| {
                let item = catalog?.by_slug(r.wfm_slug.as_deref()?)?;
                Some(BulkLine {
                    item_id: item.id.clone(),
                    name: r.name.clone(),
                    rank: price_rank(r.rank, r.max_rank),
                    platinum: state.row_price(r).map(|p| p.round().max(1.0) as u32).unwrap_or(1),
                    quantity: r.count.min(999) as u32,
                    include: true,
                })
            })
            .collect()
    }

    fn bulk_dialog_ui(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend) {
        let Some(lines) = &mut self.bulk_dialog else { return };
        let mut submit = false;
        let mut cancel = false;
        egui::Modal::new(egui::Id::unique("bulk")).show(ui.ctx(), |ui| {
            ui.set_width(560.0);
            ui.label(RichText::new("Bulk sell").font(FontId::new(24.0, theme::display_bold())));
            if state.session.is_none() {
                ui.label("Sign in to warframe.market in the Market view first.");
                cancel = ui.button("Close").clicked();
                return;
            }
            ui.label(RichText::new("Check quantities, ranks and prices. Prices default to the 48h average.").color(TEXT_SECONDARY()));
            egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                egui::Grid::new("bulk-grid").striped(true).num_columns(4).show(ui, |ui| {
                    for h in ["", "Item", "Qty", "Price"] {
                        kit::caption(ui, h);
                    }
                    ui.end_row();
                    for l in lines.iter_mut() {
                        ui.checkbox(&mut l.include, "");
                        ui.label(match l.rank {
                            Some(r) => format!("{} (R{r})", l.name),
                            None => l.name.clone(),
                        });
                        ui.add(egui::DragValue::new(&mut l.quantity).range(1..=999));
                        ui.add(egui::DragValue::new(&mut l.platinum).range(1..=100_000).suffix(" p"));
                        ui.end_row();
                    }
                });
            });
            let n = lines.iter().filter(|l| l.include).count();
            ui.horizontal(|ui| {
                submit = n > 0 && kit::primary_button(ui, &format!("  Create {n} listings  ")).clicked();
                cancel = kit::ghost_button(ui, "Cancel").clicked();
            });
        });
        if submit {
            let orders = lines
                .iter()
                .filter(|l| l.include)
                .map(|l| NewOrder {
                    item_id: l.item_id.clone(),
                    order_type: OrderType::Sell,
                    platinum: l.platinum,
                    quantity: l.quantity,
                    rank: l.rank,
                    visible: true,
                })
                .collect();
            backend.create_orders(orders);
            self.bulk_selected.clear();
            self.bulk_mode = false;
        }
        if submit || cancel {
            self.bulk_dialog = None;
        }
    }

    fn sets_ui(&mut self, ui: &mut Ui, state: &AppState) {
        let (Some(inv), Some(db)) = (&state.inventory, &state.db) else { return };
        let empty = Catalog::default();
        let catalog = state.catalog.as_deref().unwrap_or(&empty);
        ui.horizontal(|ui| {
            kit::search(ui, &mut self.search, "Search sets...", 240.0);
            ui.checkbox(&mut self.show_incomplete_sets, "Show incomplete sets");
        });
        ui.add_space(6.0);
        let q = self.search.trim().to_lowercase();
        let sets: Vec<_> = full_sets::compute(inv, db, catalog, self.show_incomplete_sets)
            .into_iter()
            .filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q))
            .collect();
        if sets.is_empty() {
            kit::empty(ui, "No sets", "No complete sets can be put together from the parts you own.");
            return;
        }
        kit::card_grid(ui, 300.0, 170.0, sets.len(), |ui, i| {
            let set = &sets[i];
            let color = if set.complete > 0 { Some(alpha(POSITIVE(), 0.45)) } else { None };
            kit::grid_card(ui, color, |ui| {
                egui::Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        kit::remote_image(ui, db.image_url(&set.result).as_deref(), vec2(40.0, 40.0));
                        ui.vertical(|ui| {
                            kit::card_title(ui, &set.name);
                            ui.horizontal(|ui| {
                                let c = if set.complete > 0 { POSITIVE() } else { WARNING() };
                                kit::pill(ui, &format!("{} complete", set.complete), c);
                                if let Some(slug) = &set.slug {
                                    kit::plat(ui, state.price(slug, None), 13.0);
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                    for p in &set.parts {
                        ui.horizontal(|ui| {
                            ui.add(egui::Label::new(RichText::new(&p.name).size(12.5).color(TEXT_SECONDARY())).truncate());
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let ok = p.owned >= p.needed;
                                ui.label(RichText::new(if ok { "✔" } else { "✖" }).size(11.0).color(if ok { POSITIVE() } else { NEGATIVE() }));
                                ui.label(RichText::new(format!("{}/{}", p.owned, p.needed)).size(12.5));
                            });
                        });
                    }
                });
            });
        });
    }
}

/// Gold-outlined "WTS R3 / 15p" box.
fn price_box(ui: &mut Ui, label: &str, value: Option<&str>) {
    egui::Frame::new()
        .fill(alpha(ACCENT(), 0.06))
        .stroke(Stroke::new(1.0, alpha(ACCENT(), 0.45)))
        .corner_radius(RADIUS_SM())
        .inner_margin(Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(label).size(10.5).color(TEXT_SECONDARY()));
            ui.label(RichText::new(value.unwrap_or("-")).family(theme::body_strong()).color(if value.is_some() { ACCENT_BRIGHT() } else { TEXT_MUTED() }));
        });
}
