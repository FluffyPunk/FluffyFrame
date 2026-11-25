//! Rivens (original: `src/views/RivensView.svelte`, `RivenFinder.svelte`,
//! `RivenPolarityIcon.svelte`). The OCR riven scanner overlay is not ported.

use std::sync::Arc;

use egui::{Align, Align2, Color32, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use fluffyframe_core::services::inventory::Inventory;
use fluffyframe_core::services::riven_good_rolls::AttrGrade;
use fluffyframe_core::services::rivens::{self, DecodedRiven, MAX_RANK, OwnedRiven};
use fluffyframe_core::services::wfm_riven_search as rivens_wfm;

use crate::assets;
use crate::components::{format, ui as kit};
use crate::ipc::{Backend, MarketCmd};
use crate::stores::AppState;
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Unveiled,
    Veiled,
    Finder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Sort {
    #[default]
    Name,
    Disposition,
    Rerolls,
    Grade,
    AttrGrade,
}

impl Sort {
    const ALL: [(Sort, &'static str); 5] = [
        (Sort::Name, "Name"),
        (Sort::Disposition, "Disposition"),
        (Sort::Rerolls, "Rerolls"),
        (Sort::Grade, "Grade"),
        (Sort::AttrGrade, "Attribute grade"),
    ];
}

const TYPES: [&str; 8] = ["All", "Rifle", "Shotgun", "Pistol", "Melee", "Archgun", "Kitgun", "Zaw"];
const GRADES: [&str; 6] = ["All", "S", "A", "B", "C", "F"];

/// Riven card template is 316×400; cards grow to 288px wide like the original.
const CARD_RATIO: f32 = 400.0 / 316.0;
const CARD_MAX_W: f32 = 288.0;
const PIP: Color32 = Color32::from_rgb(0x5e, 0xc8, 0xff);
const REROLL: Color32 = Color32::from_rgb(0xf0, 0x6d, 0xff);

/// Decoded rivens, rebuilt when inventory, game data or the sheet change.
struct Decoded {
    key: [usize; 3],
    list: Vec<(OwnedRiven, DecodedRiven)>,
}

#[derive(Default)]
pub struct RivensView {
    tab: Tab,
    search: String,
    sort: Sort,
    descending: bool,
    type_filter: &'static str,
    grade_filter: &'static str,
    attr_filter: Option<AttrGrade>,
    decoded: Option<Decoded>,
    /// Riven whose detail window is open (index into the decoded list).
    open: Option<usize>,
    /// Detail window: hide bidding auctions, keep direct sales (off by default).
    direct_only: bool,
    show_all_listings: bool,
    finder: super::riven_finder::FinderView,
}

fn ptr<T>(a: &Option<Arc<T>>) -> usize {
    a.as_ref().map(|a| Arc::as_ptr(a) as usize).unwrap_or(0)
}

/// Overall grade order for sorting: S highest.
fn grade_rank(g: &str) -> usize {
    ["S", "A+", "A", "A-", "B+", "B", "B-", "C+", "C", "C-", "F"].iter().position(|x| *x == g).map(|i| 11 - i).unwrap_or(0)
}

fn grade_color(g: &str) -> Color32 {
    match g.chars().next() {
        Some('S') => Color32::from_rgb(0x4a, 0xde, 0x80),
        Some('A') => Color32::from_rgb(0x6a, 0xab, 0x7a),
        Some('B') => Color32::from_rgb(0xfa, 0xcc, 0x15),
        Some('C' | 'D') => Color32::from_rgb(0xf9, 0x73, 0x16),
        Some('F') => Color32::from_rgb(0xef, 0x44, 0x44),
        _ => Color32::from_rgb(0x8b, 0x93, 0xa5),
    }
}

fn attr_color(g: AttrGrade) -> Color32 {
    match g {
        AttrGrade::Great => grade_color("S"),
        AttrGrade::Good => grade_color("A"),
        AttrGrade::Ok => grade_color("B"),
        AttrGrade::Bad => grade_color("F"),
    }
}

/// Text with the original's heavy dark halo so it reads over the card art.
fn shadow_text(ui: &Ui, pos: Pos2, align: Align2, text: impl ToString, font: FontId, color: Color32) -> Rect {
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, color);
    let rect = align.anchor_size(pos, galley.size());
    paint_shadowed(ui, rect.min, galley, color);
    rect
}

fn paint_shadowed(ui: &Ui, min: Pos2, galley: Arc<egui::Galley>, color: Color32) {
    let painter = ui.painter();
    let shadow = Color32::from_black_alpha(200);
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(0.0, 2.0)] {
        painter.galley_with_override_text_color(min + d, galley.clone(), shadow);
    }
    painter.galley(min, galley, color);
}

impl RivensView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend) {
        if state.riven_weapons.is_none() && !state.riven_weapons_requested {
            state.riven_weapons_requested = true;
            backend.market(MarketCmd::RivenWeapons);
        }
        if self.type_filter.is_empty() {
            self.type_filter = "All";
            self.grade_filter = "All";
        }
        self.refresh_decoded(state);
        let inv = state.inventory.clone();
        let unveiled = self.decoded.as_ref().map(|d| d.list.len()).unwrap_or(0);
        let veiled_count = inv.as_ref().map(|i| veiled_challenges(i).len() as u64 + unrevealed(i).iter().map(|(_, n)| n).sum::<u64>()).unwrap_or(0);

        kit::page_title(ui, "Rivens", None);
        kit::tabs(
            ui,
            &mut self.tab,
            &[
                (Tab::Unveiled, format!("Unveiled ({unveiled})")),
                (Tab::Veiled, format!("Veiled ({veiled_count})")),
                (Tab::Finder, "Riven Finder".into()),
            ],
        );
        let mut lookup = None;
        match self.tab {
            Tab::Unveiled => self.unveiled(ui, state, backend, &mut lookup),
            Tab::Veiled => veiled(ui, inv.as_deref()),
            Tab::Finder => self.finder.ui(ui, state, backend),
        }
        if let Some((slug, name)) = lookup {
            let melee = state
                .riven_weapons
                .as_ref()
                .and_then(|w| w.weapons.iter().find(|x| x.slug == slug))
                .and_then(|w| w.riven_type.as_deref())
                .is_some_and(|t| t == "melee" || t == "zaw");
            self.finder.open_weapon(slug, name, melee, backend);
            self.tab = Tab::Finder;
        }
    }

    fn refresh_decoded(&mut self, state: &AppState) {
        let key = [ptr(&state.inventory), ptr(&state.db), ptr(&state.good_rolls)];
        if self.decoded.as_ref().is_some_and(|d| d.key == key) {
            return;
        }
        self.open = None;
        let (Some(inv), Some(db)) = (&state.inventory, &state.db) else {
            self.decoded = None;
            return;
        };
        let list = inv
            .rivens
            .iter()
            .filter(|r| !r.is_veiled())
            .filter_map(|r| Some((r.clone(), rivens::decode(r, &db.rivens, state.good_rolls.as_deref())?)))
            .collect();
        self.decoded = Some(Decoded { key, list });
    }

    /// warframe.market riven slug for a riven's weapon.
    fn slug_for(&self, state: &AppState, riven: &OwnedRiven) -> Option<String> {
        Some(state.riven_weapons.as_ref()?.by_game_ref(riven.weapon.as_deref()?)?.slug.clone())
    }

    fn unveiled(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend, lookup: &mut Option<(String, String)>) {
        if state.inventory.is_none() {
            kit::empty(ui, "No inventory", "Load an inventory to see your rivens.");
            return;
        }
        let Some(decoded) = self.decoded.take() else {
            ui.spinner();
            return;
        };
        self.toolbar(ui);
        ui.add_space(8.0);

        let q = self.search.trim().to_lowercase();
        let mut list: Vec<(usize, &(OwnedRiven, DecodedRiven))> = decoded
            .list
            .iter()
            .enumerate()
            .filter(|(_, (r, _))| self.type_filter == "All" || r.riven_type == self.type_filter)
            .filter(|(_, (_, d))| self.grade_filter == "All" || d.grade.starts_with(self.grade_filter))
            .filter(|(_, (_, d))| self.attr_filter.is_none() || d.attr_grade == self.attr_filter)
            .filter(|(_, (_, d))| {
                q.is_empty()
                    || d.weapon_name.to_lowercase().contains(&q)
                    || d.suffix.to_lowercase().contains(&q)
                    || d.stats.iter().any(|s| s.name.to_lowercase().contains(&q))
            })
            .collect();
        list.sort_by(|(_, (ra, a)), (_, (rb, b))| {
            let ord = match self.sort {
                Sort::Name => a.weapon_name.to_lowercase().cmp(&b.weapon_name.to_lowercase()).then(a.suffix.cmp(&b.suffix)),
                Sort::Disposition => a.disposition.total_cmp(&b.disposition),
                Sort::Rerolls => ra.rerolls.cmp(&rb.rerolls),
                Sort::Grade => grade_rank(a.grade).cmp(&grade_rank(b.grade)),
                Sort::AttrGrade => a.attr_grade.cmp(&b.attr_grade),
            };
            if self.descending { ord.reverse() } else { ord }
        });

        if list.is_empty() {
            kit::empty(ui, "No rivens", "No unveiled rivens match these filters.");
        }
        let mut clicked: Option<usize> = None;
        kit::grid_sections(
            ui,
            vec![
                kit::GridSection::new(240.0, 0.0, list.len(), |ui, i| {
                    let (index, (riven, d)) = list[i];
                    if riven_card(ui, riven, d) {
                        clicked = Some(index);
                    }
                })
                .aspect(CARD_RATIO, CARD_MAX_W)
                .gap(20.0),
            ],
        );
        if let Some(i) = clicked {
            self.open = Some(i);
            self.show_all_listings = false;
            if let Some(slug) = self.slug_for(state, &decoded.list[i].0) {
                backend.market(MarketCmd::RivenAuctions(slug));
            }
        }
        if let Some(i) = self.open
            && let Some((riven, d)) = decoded.list.get(i)
        {
            self.detail_modal(ui.ctx(), state, riven, d, lookup);
        }
        self.decoded = Some(decoded);
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            kit::search(ui, &mut self.search, "Search...", 220.0);
            kit::labelled(ui, "Sort", |ui| {
                let label = Sort::ALL.iter().find(|(s, _)| *s == self.sort).map(|(_, l)| *l).unwrap_or("");
                egui::ComboBox::from_id_salt("riven-sort").width(130.0).selected_text(label).show_ui(ui, |ui| {
                    for (s, l) in Sort::ALL {
                        ui.selectable_value(&mut self.sort, s, l);
                    }
                });
            });
            if kit::chip(ui, true, if self.descending { "⬇" } else { "⬆" }).on_hover_text("Sort direction").clicked() {
                self.descending = !self.descending;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let attr_label = self.attr_filter.map(AttrGrade::label).unwrap_or("All");
                egui::ComboBox::from_id_salt("riven-attr").width(80.0).selected_text(attr_label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.attr_filter, None, "All");
                    for g in AttrGrade::ALL {
                        ui.selectable_value(&mut self.attr_filter, Some(g), g.label());
                    }
                });
                ui.label(RichText::new("Attribute grade").size(11.5).color(TEXT_MUTED()));
                egui::ComboBox::from_id_salt("riven-grade").width(60.0).selected_text(self.grade_filter).show_ui(ui, |ui| {
                    for g in GRADES {
                        ui.selectable_value(&mut self.grade_filter, g, g);
                    }
                });
                ui.label(RichText::new("Grade").size(11.5).color(TEXT_MUTED()));
                let mut t = self.type_filter;
                let options: Vec<(&'static str, &str)> = TYPES.iter().rev().map(|t| (*t, *t)).collect();
                if kit::segmented(ui, &mut t, &options) {
                    self.type_filter = t;
                }
            });
        });
    }
}

/// "●●●●○" (original: `dispoStars`).
fn dispo_stars(d: f64) -> &'static str {
    match d {
        d if d >= 1.3 => "●●●●●",
        d if d >= 1.1 => "●●●●○",
        d if d >= 0.9 => "●●●○○",
        d if d >= 0.7 => "●●○○○",
        _ => "●○○○○",
    }
}

fn section_title(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(text).font(FontId::new(16.0, theme::display_bold())).color(TEXT()));
    ui.add_space(4.0);
}

fn summary_tile(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(BG_RAISED())
        .stroke(Stroke::new(1.0, BORDER_SUBTLE()))
        .corner_radius(RADIUS_MD())
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(width);
            ui.horizontal(add);
        });
}

fn tag_chip(ui: &mut Ui, text: &str, matched: bool) {
    let (fill, color) = if matched { (alpha(POSITIVE(), 0.15), POSITIVE()) } else { (Color32::TRANSPARENT, TEXT_MUTED()) };
    egui::Frame::new().fill(fill).corner_radius(RADIUS_SM()).inner_margin(Margin::symmetric(6, 2)).show(ui, |ui| {
        let text = RichText::new(text).size(12.0).color(color);
        ui.label(if matched { text.family(theme::body_strong()) } else { text });
    });
}

impl RivensView {
    /// Riven details (original: `RivenDetailModal.svelte`). Listing a riven on
    /// warframe.market from here is not ported.
    fn detail_modal(&mut self, ctx: &egui::Context, state: &AppState, riven: &OwnedRiven, d: &DecodedRiven, lookup: &mut Option<(String, String)>) {
        let mut close = false;
        let frame = egui::Frame::new()
            .fill(BG_SURFACE())
            .stroke(Stroke::new(1.0, BORDER_STRONG()))
            .corner_radius(RADIUS_LG())
            .inner_margin(Margin::same(20));
        let max_h = ctx.content_rect().height() * 0.85;
        let resp = egui::Modal::new(egui::Id::unique("riven-detail")).frame(frame).show(ctx, |ui| {
            ui.set_width(820.0);
            // Header.
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} {}", d.weapon_name, d.suffix).trim()).font(FontId::new(26.0, theme::display_bold())).color(TEXT()));
                ui.label(RichText::new(d.grade).font(FontId::new(22.0, theme::display_bold())).color(grade_color(d.grade)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("✕").clicked() {
                        close = true;
                    }
                    if let Some(slug) = self.slug_for(state, riven)
                        && kit::ghost_button(ui, "Riven Finder").on_hover_text("Search live auctions for this weapon").clicked()
                    {
                        *lookup = Some((slug, d.weapon_name.clone()));
                        close = true;
                    }
                });
            });
            ui.horizontal_wrapped(|ui| {
                kit::pill(ui, &format!("{} Riven", riven.riven_type), TEXT_SECONDARY());
                ui.label(RichText::new(format!("{} {:.2}", dispo_stars(d.disposition), d.disposition)).color(ACCENT()))
                    .on_hover_text(format!("Disposition {:.3}", d.disposition));
                ui.label(RichText::new(format!("Rerolls {}", riven.rerolls)).color(REROLL));
                ui.label(RichText::new(format!("Rank {}/{}", riven.rank, MAX_RANK)).color(TEXT_SECONDARY()));
                if riven.mastery_req > 0 {
                    ui.label(RichText::new(format!("MR {}", riven.mastery_req)).color(TEXT_SECONDARY()));
                }
            });
            ui.add_space(10.0);

            egui::ScrollArea::vertical().max_height(max_h - 120.0).min_scrolled_height(max_h - 120.0).show(ui, |ui| {
                // Summary tiles.
                let half = (ui.available_width() - 40.0) / 2.0;
                ui.horizontal_top(|ui| {
                    summary_tile(ui, half, |ui| {
                        kit::caption(ui, "Roll quality");
                        ui.label(RichText::new(d.grade).font(FontId::new(22.0, theme::display_bold())).color(grade_color(d.grade)));
                        ui.label(RichText::new(format!("{}% perfect", (d.perfect * 100.0).round())).color(TEXT_SECONDARY()));
                    });
                    summary_tile(ui, half, |ui| {
                        kit::caption(ui, "Attributes");
                        match d.attr_grade {
                            Some(g) => ui.label(RichText::new(g.label()).font(FontId::new(22.0, theme::display_bold())).color(attr_color(g))),
                            None => ui.label(RichText::new("?").font(FontId::new(22.0, theme::display_bold())).color(TEXT_MUTED())).on_hover_text("No good-roll data for this weapon"),
                        };
                        let (b, c) = (riven.buffs.len(), riven.curses.len());
                        let mut text = format!("{b} buff{}", if b == 1 { "" } else { "s" });
                        if c > 0 {
                            text += ", 1 curse";
                        }
                        ui.label(RichText::new(text).color(TEXT_SECONDARY()));
                        if let Some(endo) = d.dissolve_hint(riven) {
                            kit::icon(ui, assets::ENDO, 14.0);
                            ui.label(RichText::new(format!("Dissolves for {endo}")).size(12.5).color(TEXT_SECONDARY()));
                        }
                    });
                });

                // Stats with roll bars.
                section_title(ui, "Attributes");
                for stat in &d.stats {
                    let color = if stat.positive { POSITIVE() } else { NEGATIVE() };
                    egui::Frame::new().fill(alpha(color, 0.06)).corner_radius(RADIUS_MD()).inner_margin(Margin::symmetric(12, 8)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(stat.value_text()).font(FontId::new(18.0, theme::display_bold())).color(color));
                            if let Some(img) = assets::element_icon(&stat.tag) {
                                kit::icon(ui, img, 16.0);
                            }
                            ui.label(RichText::new(&stat.name).size(14.5).color(TEXT()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(stat.grade).family(theme::body_strong()).color(grade_color(stat.grade)));
                                let quality = if stat.positive { stat.roll } else { 1.0 - stat.roll };
                                ui.allocate_ui(vec2(140.0, 10.0), |ui| {
                                    ui.set_width(140.0);
                                    kit::progress(ui, quality as f32, color, 5.0);
                                })
                                .response
                                .on_hover_text(format!("{:.0}% of the roll range", quality * 100.0));
                            });
                        });
                    });
                    ui.add_space(4.0);
                }

                // Community good rolls for this weapon.
                section_title(ui, &format!("Best attributes for {}", d.weapon_name));
                let data = state.good_rolls.as_ref().zip(state.db.as_ref()).and_then(|(g, db)| Some((g.get(&d.weapon_name, &db.rivens)?, db)));
                match data {
                    Some((data, db)) => {
                        use fluffyframe_core::services::riven_good_rolls::sheet_tag;
                        let label = |tag: &str| db.rivens.entry(&riven.item_type, tag).map(|e| e.name.clone()).unwrap_or_else(|| tag.to_string());
                        let buffs: Vec<String> = riven.buffs.iter().map(|b| sheet_tag(&b.tag)).collect();
                        let curses: Vec<String> = riven.curses.iter().map(|c| sheet_tag(&c.tag)).collect();
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Wanted:").size(12.5).color(TEXT_SECONDARY()));
                            for tag in data.wanted() {
                                tag_chip(ui, &label(tag), buffs.iter().any(|b| b == tag));
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("Accepted curses:").size(12.5).color(TEXT_SECONDARY()));
                            if data.accepted_bad.is_empty() {
                                ui.label(RichText::new("none").size(12.0).color(TEXT_MUTED()));
                            }
                            for tag in &data.accepted_bad {
                                tag_chip(ui, &label(tag), curses.iter().any(|c| c == tag));
                            }
                        });
                    }
                    None => {
                        ui.label(RichText::new("No good-roll data for this weapon.").color(TEXT_MUTED()));
                    }
                }

                // Similar live listings.
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Similar on warframe.market").font(FontId::new(16.0, theme::display_bold())).color(TEXT()));
                    if kit::chip(ui, !self.direct_only, "Auctions").on_hover_text("Include bidding auctions, not only direct sales").clicked() {
                        self.direct_only = !self.direct_only;
                    }
                });
                ui.add_space(4.0);
                let slug = self.slug_for(state, riven);
                match slug.as_ref().and_then(|s| state.riven_auctions.get(s)) {
                    None if slug.is_some() => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(RichText::new("Searching auctions…").color(TEXT_MUTED()));
                        });
                    }
                    None => {
                        ui.label(RichText::new("warframe.market has no riven listings for this weapon.").color(TEXT_MUTED()));
                    }
                    Some(auctions) => {
                        let mine: Vec<&str> = d.stats.iter().map(|s| s.tag.as_str()).collect();
                        let mut similar: Vec<(u32, Vec<bool>, &rivens_wfm::RivenAuction)> = auctions
                            .iter()
                            .filter(|a| !self.direct_only || a.direct_sell)
                            .map(|a| {
                                let names: Vec<String> = a.attributes.iter().map(|(n, _, _)| n.clone()).collect();
                                let (pct, matched) = rivens::similarity(&mine, &names);
                                (pct, matched, a)
                            })
                            .filter(|(pct, _, _)| *pct >= 25)
                            .collect();
                        similar.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.price().unwrap_or(u32::MAX).cmp(&b.2.price().unwrap_or(u32::MAX))));
                        if similar.is_empty() {
                            ui.label(RichText::new("No similar rivens listed.").color(TEXT_MUTED()));
                        }
                        let shown = if self.show_all_listings { similar.len() } else { similar.len().min(20) };
                        for (pct, matched, a) in &similar[..shown] {
                            kit::raised(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal_top(|ui| {
                                    let pct_color = if *pct >= 75 { POSITIVE() } else if *pct >= 40 { WARNING() } else { NEGATIVE() };
                                    kit::pill(ui, &format!("{pct}%"), pct_color);
                                    kit::plat(ui, a.price().map(f64::from), 14.0);
                                    ui.label(RichText::new(format!("⟳ {}", a.rerolls)).size(12.0).color(REROLL));
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        for ((attr, value, positive), hit) in a.attributes.iter().zip(matched) {
                                            let color = if *positive { POSITIVE() } else { NEGATIVE() };
                                            let text = RichText::new(format!("{value:+} {}", format::title_case(&attr.replace('_', " ")))).size(12.0).color(if *hit { color } else { alpha(color, 0.4) });
                                            ui.label(if *hit { text } else { text.strikethrough() });
                                        }
                                    });
                                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                                        if ui.small_button("WFM ↗").clicked() {
                                            ui.ctx().open_url(egui::OpenUrl::new_tab(a.url()));
                                        }
                                        ui.label(RichText::new(&a.owner).size(12.0).color(TEXT_MUTED()));
                                    });
                                });
                            });
                            ui.add_space(3.0);
                        }
                        if similar.len() > 20 {
                            let label = if self.show_all_listings { "Show fewer".to_string() } else { format!("Show all {} · {} more", similar.len(), similar.len() - 20) };
                            if kit::ghost_button(ui, &label).clicked() {
                                self.show_all_listings = !self.show_all_listings;
                            }
                        }
                    }
                }
            });
        });
        if close || resp.should_close() {
            self.open = None;
        }
    }
}

/// One riven drawn on the card template (original: RivensView's full-size card).
/// Returns true when the card body is clicked.
fn riven_card(ui: &mut Ui, riven: &OwnedRiven, d: &DecodedRiven) -> bool {
    let cell = ui.max_rect();
    let w = cell.width().min(CARD_MAX_W);
    let base = Rect::from_min_size(pos2(cell.center().x - w / 2.0, cell.top()), vec2(w, w * CARD_RATIO));
    // One id per hit area: `next_auto_id` does not advance between `interact`
    // calls, and a shared id would let the last (small) area replace the card's.
    let card_id = ui.next_auto_id();
    let resp = ui.interact(base, card_id.with("card"), Sense::click());
    // Lift on hover, like the original's translate-y.
    let card = if resp.hovered() { base.translate(vec2(0.0, -4.0)) } else { base };
    let h = card.height();
    // The original keeps its card text at fixed sizes (12/16/20px) at any width.
    let s = 1.0;
    let at = |fx: f32, fy: f32| pos2(card.left() + card.width() * fx, card.top() + h * fy);
    egui::Image::new(assets::RIVEN_TEMPLATE).paint_at(ui, card);

    // Grade badges, top right.
    shadow_text(ui, at(0.85, 0.10), Align2::RIGHT_TOP, d.grade, FontId::new(16.0 * s, theme::display_bold()), grade_color(d.grade));
    match d.attr_grade {
        Some(g) => {
            shadow_text(ui, at(0.87, 0.155), Align2::RIGHT_TOP, g.label().to_uppercase(), FontId::new(9.6 * s, theme::body_strong()), attr_color(g));
        }
        None => {
            let r = shadow_text(ui, at(0.87, 0.155), Align2::RIGHT_TOP, "?", FontId::new(9.6 * s, theme::body_strong()), TEXT_MUTED());
            ui.interact(r, card_id.with("attr"), Sense::hover()).on_hover_text("No good-roll data for this weapon");
        }
    }
    if let Some(endo) = d.dissolve_hint(riven) {
        let font = FontId::new(9.6 * s, theme::body_strong());
        let galley = ui.painter().layout_no_wrap(endo.to_string(), font, TEXT_SECONDARY());
        let icon = 10.0 * s;
        let size = vec2(icon + 4.0 + galley.size().x + 12.0, galley.size().y + 4.0);
        let chip = Align2::RIGHT_TOP.anchor_size(at(0.88, 0.195), size);
        ui.painter().rect(chip, 99.0, alpha(BG_DEEP(), 0.8), Stroke::new(1.0, BORDER()), egui::StrokeKind::Inside);
        egui::Image::new(assets::ENDO).paint_at(ui, Rect::from_center_size(pos2(chip.left() + 6.0 + icon / 2.0, chip.center().y), vec2(icon, icon)));
        ui.painter().galley(pos2(chip.left() + 6.0 + icon + 4.0, chip.top() + 2.0), galley, TEXT_SECONDARY());
        ui.interact(chip, card_id.with("endo"), Sense::hover()).on_hover_text(format!("Dissolves for {endo} Endo"));
    }

    // Weapon name and generated suffix, centred.
    let (left, right) = (card.left() + w * 0.13, card.right() - w * 0.11);
    let mut job = egui::text::LayoutJob::default();
    job.append(&d.weapon_name, 0.0, egui::TextFormat::simple(FontId::new(20.0 * s, theme::display_bold()), TEXT()));
    if !d.suffix.is_empty() {
        job.append(&d.suffix, 5.0 * s, egui::TextFormat::simple(FontId::new(14.0 * s, theme::body_strong()), TEXT_SECONDARY()));
    }
    job.wrap.max_width = right - left;
    job.wrap.max_rows = 2;
    job.halign = Align::Center;
    let name = ui.painter().layout_job(job);
    let extra_rows = name.rows.len().saturating_sub(1) as f32;
    paint_shadowed(ui, pos2((left + right) / 2.0, card.top() + h * 0.51), name, TEXT());

    // Stats, stepped at the original's 1.05 line height rather than the font's
    // full row height, so four stats clear the MR bar.
    let pitch = 16.0 * s * 1.05;
    let mut y = card.top() + h * 0.59 + extra_rows * 22.0 * s;
    let font = FontId::new(16.0 * s, theme::display());
    let icon = 16.0 * s;
    for stat in &d.stats {
        let color = if stat.positive { POSITIVE() } else { NEGATIVE() };
        let value = ui.painter().layout_no_wrap(stat.value_text(), FontId::new(16.0 * s, theme::display_bold()), color);
        let element = assets::element_icon(&stat.tag);
        let fixed = value.size().x + 4.0 + if element.is_some() { icon + 3.0 } else { 0.0 };
        let mut job = egui::text::LayoutJob::simple_singleline(stat.name.clone(), font.clone(), TEXT());
        job.wrap.max_width = (right - left - fixed).max(10.0);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let label = ui.painter().layout_job(job);
        let total = fixed + label.size().x;
        let mut x = (left + right) / 2.0 - total / 2.0;
        let row_h = value.size().y;
        let vx = value.size().x;
        // Centre the taller egui row on the pitch line.
        let top = y - (row_h - pitch) / 2.0;
        paint_shadowed(ui, pos2(x, top), value, color);
        x += vx + 4.0;
        if let Some(img) = element {
            egui::Image::new(img).paint_at(ui, Rect::from_min_size(pos2(x, y + (pitch - icon) / 2.0), vec2(icon, icon)));
            x += icon + 3.0;
        }
        paint_shadowed(ui, pos2(x, top), label, TEXT());
        y += pitch;
    }

    // MR · polarity · rerolls: a row starting at 83.5% with 12px text centred in it.
    let meta_y = card.top() + h * 0.835 + 6.0;
    let meta_font = FontId::new(12.0 * s, theme::body_strong());
    shadow_text(ui, pos2(card.left() + w * 0.22, meta_y), Align2::LEFT_CENTER, format!("MR {}", riven.mastery_req), meta_font.clone(), TEXT_SECONDARY());
    if let Some(icon) = riven.polarity.as_deref().and_then(assets::polarity_icon) {
        let r = Rect::from_center_size(pos2(card.center().x, meta_y - 2.0), vec2(14.0 * s, 14.0 * s));
        egui::Image::new(icon).tint(TEXT()).paint_at(ui, r);
    }
    shadow_text(ui, pos2(card.right() - w * 0.22, meta_y), Align2::RIGHT_CENTER, format!("⟳ {}", riven.rerolls), meta_font, REROLL);

    // Rank pips.
    let pip = 8.0 * s;
    let gap = 4.0 * s;
    let total = MAX_RANK as f32 * pip + (MAX_RANK - 1) as f32 * gap;
    let mut x = card.center().x - total / 2.0;
    let py = card.top() + h * 0.94;
    for i in 0..MAX_RANK {
        let r = Rect::from_min_size(pos2(x, py), vec2(pip, pip));
        if i < riven.rank {
            for (grow, a) in [(4.0, 0.12), (2.0, 0.25)] {
                ui.painter().rect_filled(r.expand(grow * s), 2.0, alpha(PIP, a));
            }
            ui.painter().rect_filled(r, 1.0, PIP);
        } else {
            ui.painter().rect(r, 1.0, BG_SURFACE(), Stroke::new(1.0, BORDER_SUBTLE()), egui::StrokeKind::Inside);
        }
        x += pip + gap;
    }

    // Copy chat tag, top left (above the card so it takes the click).
    let copy = Rect::from_min_size(at(0.13, 0.09), vec2(30.0 * s, 30.0 * s));
    let copy_resp = ui.interact(copy, card_id.with("copy"), Sense::click());
    let fill = if copy_resp.hovered() { BG_RAISED() } else { alpha(BG_DEEP(), 0.85) };
    ui.painter().rect(copy, 4.0, fill, Stroke::new(1.0, BORDER()), egui::StrokeKind::Inside);
    let glyph = if copy_resp.hovered() { TEXT() } else { TEXT_SECONDARY() };
    let g = copy.shrink(8.0 * s);
    ui.painter().rect_stroke(Rect::from_min_max(g.min + vec2(g.width() * 0.35, g.height() * 0.35), g.max), 2.0, Stroke::new(1.5, glyph), egui::StrokeKind::Inside);
    ui.painter().rect_stroke(Rect::from_min_max(g.min, g.max - vec2(g.width() * 0.35, g.height() * 0.35)), 2.0, Stroke::new(1.5, glyph), egui::StrokeKind::Inside);
    let tag = format!("[{} {}]", d.weapon_name, d.suffix).replace(" ]", "]");
    if copy_resp.on_hover_text(format!("Copy chat tag {tag}")).clicked() {
        ui.ctx().copy_text(tag);
    }

    resp.on_hover_text("Open riven details").clicked()
}

/// Challenge-veiled rivens (in `Upgrades`, unlock challenge attached).
fn veiled_challenges(inv: &Inventory) -> Vec<&OwnedRiven> {
    inv.rivens.iter().filter(|r| r.is_veiled()).collect()
}

/// Stacked veiled rivens never equipped (`RawUpgrades`), counted per type.
fn unrevealed(inv: &Inventory) -> Vec<(&'static str, u64)> {
    let mut out: Vec<(&'static str, u64)> = Vec::new();
    for it in inv.items.iter().filter(|it| it.unique_name.contains("/Randomized/") && it.unique_name.rsplit('/').next().unwrap_or("").starts_with("Raw")) {
        let label = rivens::riven_type_label(&it.unique_name);
        match out.iter_mut().find(|(l, _)| *l == label) {
            Some((_, n)) => *n += it.count,
            None => out.push((label, it.count)),
        }
    }
    out
}

/// Veiled tab: challenge groups with progress, then unrevealed stacks
/// (original: RivensView's veiled section and `groupVeiledRivens`).
fn veiled(ui: &mut Ui, inv: Option<&Inventory>) {
    let Some(inv) = inv else {
        kit::empty(ui, "No inventory", "Load an inventory to see your rivens.");
        return;
    };
    let challenged = veiled_challenges(inv);
    let unseen = unrevealed(inv);
    if challenged.is_empty() && unseen.is_empty() {
        kit::empty(ui, "No veiled rivens", "Veiled rivens in your inventory appear here.");
        return;
    }
    let mut groups: Vec<(String, Vec<&OwnedRiven>)> = Vec::new();
    for r in &challenged {
        let label = r.challenge.as_ref().map(|c| c.group.clone()).unwrap_or_else(|| "Challenge not assigned".into());
        match groups.iter_mut().find(|(l, _)| *l == label) {
            Some((_, list)) => list.push(r),
            None => groups.push((label, vec![r])),
        }
    }
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        for (label, list) in &groups {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).font(FontId::new(15.0, theme::display_bold())));
                ui.label(RichText::new(format!("x{}", list.len())).color(ACCENT()).family(theme::body_strong()));
            });
            for r in list {
                kit::raised(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        kit::icon(ui, assets::RIVEN, 28.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(format!("{} Riven Mod", r.riven_type)).family(theme::body_strong()));
                            match &r.challenge {
                                Some(c) => {
                                    ui.label(RichText::new(&c.description).size(12.5).color(TEXT_SECONDARY()));
                                    if let (Some(p), Some(req)) = (c.progress, c.required) {
                                        ui.horizontal(|ui| {
                                            ui.allocate_ui(vec2(220.0, 8.0), |ui| {
                                                ui.set_width(220.0);
                                                kit::progress(ui, p as f32 / req.max(1) as f32, ACCENT(), 4.0);
                                            });
                                            ui.label(RichText::new(format!("{p} / {req}")).size(11.5).color(TEXT_MUTED()));
                                        });
                                    }
                                }
                                None => {
                                    ui.label(RichText::new("Challenge not assigned yet").size(12.5).color(TEXT_MUTED()));
                                }
                            }
                        });
                    });
                });
                ui.add_space(4.0);
            }
            ui.add_space(10.0);
        }
        if !unseen.is_empty() {
            kit::caption(ui, "Unrevealed");
            ui.add_space(4.0);
            ui.with_layout(Layout::left_to_right(Align::Min).with_main_wrap(true), |ui| {
                for (label, count) in &unseen {
                    kit::raised(ui, |ui| {
                        ui.set_width(230.0);
                        ui.horizontal(|ui| {
                            kit::icon(ui, assets::RIVEN, 28.0);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(format!("{label} Riven Mod")).family(theme::body_strong()));
                                ui.label(RichText::new("Equip to reveal its challenge").size(11.5).color(TEXT_MUTED()));
                            });
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(format!("x{count}")).color(POSITIVE()).family(theme::body_strong()));
                            });
                        });
                    });
                }
            });
        }
    });
}
