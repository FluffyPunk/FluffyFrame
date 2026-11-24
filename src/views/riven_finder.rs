//! Riven Finder tab (original: `src/components/RivenFinder.svelte`): pick a
//! weapon, describe the stats you want, and rank its live warframe.market
//! auctions by how many of them each listing has.

use egui::{Align, Color32, FontId, Layout, Margin, RichText, Stroke, Ui, vec2};
use wfhelper_core::config::game_tables;
use wfhelper_core::services::wfm_riven_search::{self as wfm, RivenAuction};

use crate::components::{format, ui as kit};
use crate::ipc::{Backend, MarketCmd};
use crate::stores::AppState;
use crate::theme::{self, *};

/// One attribute row: a wanted stat (by warframe.market name) and whether a
/// listing must have it.
#[derive(Default, Clone)]
struct Slot {
    positive: bool,
    stat: Option<&'static str>,
    required: bool,
}

pub struct FinderView {
    /// Text in the weapon box; equals `selected`'s name once one is picked.
    query: String,
    /// (slug, name, melee) of the chosen weapon.
    selected: Option<(String, String, bool)>,
    dropdown: bool,
    slots: [Slot; 4],
    price_min: String,
    price_max: String,
    rerolls_min: String,
    rerolls_max: String,
    min_similarity: String,
    require_negative: bool,
    online_only: bool,
    hide_one_plat: bool,
    /// Slug the Search button last asked for.
    searched: Option<String>,
    options: Vec<(&'static str, String)>,
}

impl Default for FinderView {
    fn default() -> Self {
        let slot = |positive| Slot { positive, ..Default::default() };
        Self {
            query: String::new(),
            selected: None,
            dropdown: false,
            slots: [slot(true), slot(true), slot(true), slot(false)],
            price_min: String::new(),
            price_max: String::new(),
            rerolls_min: String::new(),
            rerolls_max: String::new(),
            min_similarity: String::new(),
            require_negative: false,
            online_only: true,
            hide_one_plat: false,
            searched: None,
            options: wfm::stat_options(),
        }
    }
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).size(11.5).family(theme::body_strong()).color(TEXT_MUTED()));
}

fn chip(ui: &mut Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(alpha(color, 0.12))
        .stroke(Stroke::new(1.0, alpha(color, 0.45)))
        .corner_radius(RADIUS_SM())
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).family(theme::body_strong()).color(color));
        });
}

fn number_box(ui: &mut Ui, value: &mut String, hint: &str) {
    ui.add(egui::TextEdit::singleline(value).hint_text(hint).desired_width(72.0).char_limit(6));
    value.retain(|c| c.is_ascii_digit());
}

fn parse(v: &str) -> Option<u32> {
    v.parse().ok()
}

/// "ingame" / "online" count as reachable (original: `isActiveOrderStatus`).
fn is_active(status: &str) -> bool {
    matches!(status, "ingame" | "online")
}

impl FinderView {
    /// Pick a weapon and search it straight away (from a riven's details).
    pub fn open_weapon(&mut self, slug: String, name: String, melee: bool, backend: &Backend) {
        self.query = name.clone();
        self.selected = Some((slug.clone(), name, melee));
        self.dropdown = false;
        backend.market(MarketCmd::RivenAuctions(slug.clone()));
        self.searched = Some(slug);
    }

    fn label(&self, url_name: &str) -> String {
        self.options
            .iter()
            .find(|(n, _)| *n == url_name)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| format::title_case(&url_name.replace('_', " ")))
    }

    pub fn ui(&mut self, ui: &mut Ui, state: &AppState, backend: &Backend) {
        let total = ui.available_width();
        let left_w = (total - 20.0) * (1.0 / 2.4);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
                ui.set_width(left_w);
                self.weapon_column(ui, state);
            });
            ui.add_space(20.0 - ui.spacing().item_spacing.x);
            ui.vertical(|ui| self.filter_column(ui, backend));
        });
        ui.add_space(14.0);
        self.results(ui, state);
    }

    fn weapon_column(&mut self, ui: &mut Ui, state: &AppState) {
        caption(ui, "Weapon");
        let resp = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .hint_text("Type weapon name...")
                .desired_width(f32::INFINITY)
                .margin(Margin::symmetric(10, 8)),
        );
        if resp.gained_focus() {
            self.query.clear();
            self.dropdown = true;
        }
        if resp.changed() {
            self.dropdown = true;
            if self.selected.as_ref().is_some_and(|(_, n, _)| *n != self.query) {
                self.selected = None;
                self.searched = None;
            }
        }
        let picked_name = self.selected.as_ref().map(|(_, n, _)| n.as_str());
        if self.dropdown && Some(self.query.as_str()) != picked_name {
            let q = self.query.trim().to_lowercase();
            let weapons: Vec<_> = state
                .riven_weapons
                .iter()
                .flat_map(|w| w.weapons.iter())
                .filter(|w| q.is_empty() || w.name.to_lowercase().contains(&q))
                .take(50)
                .collect();
            let area = egui::Area::new(ui.scope_id().with("weapon-dropdown"))
                .order(egui::Order::Foreground)
                .fixed_pos(resp.rect.left_bottom())
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .fill(BG_RAISED())
                        .stroke(Stroke::new(1.0, BORDER_STRONG()))
                        .corner_radius(egui::CornerRadius { nw: 0, ne: 0, sw: RADIUS_MD(), se: RADIUS_MD() })
                        .show(ui, |ui| {
                            ui.set_width(resp.rect.width());
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                if weapons.is_empty() {
                                    ui.label(RichText::new("  No riven weapons match").color(TEXT_MUTED()));
                                }
                                for w in weapons {
                                    let row = ui.add_sized(
                                        [ui.available_width(), 26.0],
                                        egui::Button::new(RichText::new(&w.name).size(13.5)).frame(false),
                                    );
                                    if row.clicked() {
                                        let melee = w.riven_type.as_deref().is_some_and(|t| t == "melee" || t == "zaw");
                                        self.query = w.name.clone();
                                        self.selected = Some((w.slug.clone(), w.name.clone(), melee));
                                        self.searched = None;
                                        self.dropdown = false;
                                    }
                                }
                            });
                        });
                });
            // Close on a click anywhere else.
            if ui.input(|i| i.pointer.any_click()) && !resp.hovered() && !area.response.hovered() {
                self.dropdown = false;
            }
        }

        let Some((_, name, melee)) = &self.selected else { return };
        let data = state.good_rolls.as_ref().zip(state.db.as_ref()).and_then(|(g, db)| g.get(name, &db.rivens));
        let Some(data) = data else {
            ui.add_space(10.0);
            ui.label(RichText::new("No good-roll data for this weapon.").color(TEXT_MUTED()));
            return;
        };
        ui.add_space(10.0);
        caption(ui, "Best positives");
        ui.horizontal_wrapped(|ui| {
            for tag in data.wanted() {
                chip(ui, &game_tables::riven_stat_label(tag, *melee), POSITIVE());
            }
        });
        ui.add_space(8.0);
        caption(ui, "Best negatives");
        ui.horizontal_wrapped(|ui| {
            if data.accepted_bad.is_empty() {
                ui.label(RichText::new("none").size(12.0).color(TEXT_MUTED()));
            }
            for tag in &data.accepted_bad {
                chip(ui, &game_tables::riven_stat_label(tag, *melee), NEGATIVE());
            }
        });
    }

    fn filter_column(&mut self, ui: &mut Ui, backend: &Backend) {
        caption(ui, "Attributes");
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let color = if slot.positive { POSITIVE() } else { NEGATIVE() };
            egui::Frame::new()
                .fill(alpha(color, 0.08))
                .stroke(Stroke::new(1.0, alpha(color, 0.35)))
                .corner_radius(RADIUS_MD())
                .inner_margin(Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(if slot.positive { "+" } else { "−" }).font(FontId::new(15.0, theme::display_bold())).color(color));
                        let any = if slot.positive { "Any positive" } else { "Any negative" };
                        let current = slot.stat.map(|s| self.options.iter().find(|(n, _)| *n == s).map(|(_, l)| l.as_str()).unwrap_or(s)).unwrap_or(any);
                        let width = ui.available_width() - 64.0;
                        egui::ComboBox::from_id_salt(("finder-slot", i)).width(width).selected_text(current).height(320.0).show_ui(ui, |ui| {
                            ui.selectable_value(&mut slot.stat, None, any);
                            for (name, label) in &self.options {
                                ui.selectable_value(&mut slot.stat, Some(*name), label);
                            }
                        });
                        ui.checkbox(&mut slot.required, RichText::new("Req").size(12.0).color(TEXT_MUTED()))
                            .on_hover_text("Listings must have this stat");
                    });
                });
            ui.add_space(2.0);
        }

        ui.add_space(8.0);
        caption(ui, "Filters");
        let row_label = |ui: &mut Ui, text: &str| {
            ui.allocate_ui(vec2(70.0, 20.0), |ui| {
                ui.set_width(70.0);
                ui.label(RichText::new(text).size(12.5).color(TEXT_SECONDARY()));
            });
        };
        ui.horizontal(|ui| {
            row_label(ui, "Price");
            number_box(ui, &mut self.price_min, "Min");
            ui.label(RichText::new("-").color(TEXT_MUTED()));
            number_box(ui, &mut self.price_max, "Max");
        });
        ui.horizontal(|ui| {
            row_label(ui, "Rerolls");
            number_box(ui, &mut self.rerolls_min, "Min");
            ui.label(RichText::new("-").color(TEXT_MUTED()));
            number_box(ui, &mut self.rerolls_max, "Max");
        });
        ui.horizontal(|ui| {
            row_label(ui, "Similarity");
            number_box(ui, &mut self.min_similarity, "Min %");
            ui.label(RichText::new("%").color(TEXT_MUTED()));
        });
        ui.checkbox(&mut self.require_negative, RichText::new("Require negative stat").size(12.5).color(TEXT_SECONDARY()));
        ui.checkbox(&mut self.online_only, RichText::new("Online / in-game only").size(12.5).color(TEXT_SECONDARY()));
        ui.checkbox(&mut self.hide_one_plat, RichText::new("Hide 1p listings").size(12.5).color(TEXT_SECONDARY()));
        ui.add_space(8.0);

        let enabled = self.selected.is_some();
        let label = "   Search WFM   ";
        let clicked = ui.add_enabled_ui(enabled, |ui| kit::primary_button(ui, label)).inner.clicked();
        if clicked && let Some((slug, _, _)) = &self.selected {
            backend.market(MarketCmd::RivenAuctions(slug.clone()));
            self.searched = Some(slug.clone());
        }
    }

    /// Listings that pass the filters, with the share of selected stats they
    /// have; best match first, then cheapest (original: `filteredResults`).
    fn ranked<'a>(&self, auctions: &'a [RivenAuction]) -> Vec<(u32, &'a RivenAuction)> {
        let (pmin, pmax) = (parse(&self.price_min).unwrap_or(0), parse(&self.price_max).unwrap_or(u32::MAX));
        let (rmin, rmax) = (parse(&self.rerolls_min).unwrap_or(0), parse(&self.rerolls_max).unwrap_or(u32::MAX));
        let min_sim = parse(&self.min_similarity).unwrap_or(0);
        let chosen: Vec<&Slot> = self.slots.iter().filter(|s| s.stat.is_some()).collect();
        let has = |a: &RivenAuction, slot: &Slot| a.attributes.iter().any(|(n, _, pos)| *pos == slot.positive && Some(n.as_str()) == slot.stat);
        let mut out: Vec<(u32, &RivenAuction)> = auctions
            .iter()
            .filter(|a| {
                let price = a.price().unwrap_or(0);
                (pmin..=pmax).contains(&price)
                    && (rmin..=rmax).contains(&a.rerolls)
                    && (!self.require_negative || a.attributes.iter().any(|(_, _, pos)| !pos))
                    && (!self.online_only || is_active(&a.owner_status))
                    && (!self.hide_one_plat || price > 1)
                    && chosen.iter().filter(|s| s.required).all(|s| has(a, s))
            })
            .filter_map(|a| {
                if chosen.is_empty() {
                    return (100 >= min_sim).then_some((100, a));
                }
                let hits = chosen.iter().filter(|s| has(a, s)).count();
                let sim = ((hits as f64 / chosen.len() as f64) * 100.0).round() as u32;
                (sim > 0 && sim >= min_sim).then_some((sim, a))
            })
            .collect();
        out.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.price().unwrap_or(u32::MAX).cmp(&b.1.price().unwrap_or(u32::MAX))));
        out
    }

    fn results(&self, ui: &mut Ui, state: &AppState) {
        let Some(slug) = &self.searched else { return };
        let Some(auctions) = state.riven_auctions.get(slug) else {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(RichText::new("Searching warframe.market auctions...").color(TEXT_MUTED()));
            });
            return;
        };
        let ranked = self.ranked(auctions);
        if ranked.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                let text = if auctions.is_empty() { "No auctions found" } else { "No auctions found matching filters" };
                ui.label(RichText::new(text).color(TEXT_MUTED()));
            });
            return;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Similar rivens:").font(FontId::new(14.0, theme::display_bold())).color(TEXT_SECONDARY()));
            ui.label(RichText::new(format!("Results: {}", ranked.len())).size(12.0).color(TEXT_MUTED()));
        });
        ui.add_space(4.0);
        let tallest = ranked.iter().map(|(_, a)| a.attributes.len()).max().unwrap_or(3) as f32;
        // Header, status line, one row per stat, then the rolls/Open footer.
        kit::card_grid(ui, 180.0, 92.0 + tallest * 17.0, ranked.len(), |ui, i| {
            let (sim, a) = ranked[i];
            kit::grid_card(ui, None, |ui| {
                egui::Frame::new().inner_margin(Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{sim}%")).size(12.0).family(theme::body_strong()).color(TEXT_MUTED()));
                        ui.label(RichText::new(a.price().map(|p| format!("{p}p")).unwrap_or("-".into())).family(theme::body_strong()).color(ACCENT_BRIGHT()));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add(egui::Label::new(RichText::new(&a.owner).size(12.0).color(TEXT_MUTED())).truncate());
                        });
                    });
                    let status = match a.owner_status.as_str() {
                        "ingame" => "In game",
                        "online" => "Online",
                        "offline" => "Offline",
                        "invisible" => "Invisible",
                        _ => "Unknown",
                    };
                    ui.label(RichText::new(status.to_uppercase()).size(10.5).color(if is_active(&a.owner_status) { POSITIVE() } else { TEXT_MUTED() }));
                    for (name, _, positive) in &a.attributes {
                        let color = if *positive { POSITIVE() } else { NEGATIVE() };
                        let sign = if *positive { "+" } else { "−" };
                        ui.add(egui::Label::new(RichText::new(format!("{sign}{}", self.label(name))).size(12.0).color(color)).truncate());
                    }
                    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} rolls", a.rerolls)).size(12.0).color(TEXT_MUTED()));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui.add(egui::Button::new(RichText::new("OPEN ON WFM ↗").size(11.0).family(theme::body_strong()).color(ACCENT_BRIGHT()))).clicked() {
                                    ui.ctx().open_url(egui::OpenUrl::new_tab(a.url()));
                                }
                            });
                        });
                    });
                });
            });
        });
    }
}
