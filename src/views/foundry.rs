//! Foundry (original: `src/views/FoundryView.svelte`, `CraftingTree.svelte`).

use chrono::Utc;
use egui::{Align, Layout, Margin, RichText, Ui, vec2};
use fluffyframe_core::services::foundry::{self, FoundryRow};
use fluffyframe_core::services::inventory::level_from_xp;
use fluffyframe_core::services::item_database::ItemKind;

use super::need_inventory;
use crate::components::{format, ui as kit};
use crate::stores::AppState;
use crate::theme::{self, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    All,
    InProgress,
    Warframe,
    Primary,
    Secondary,
    Melee,
    Archwing,
    Companion,
    Misc,
}

impl Tab {
    const ALL: [Tab; 9] = [Tab::All, Tab::InProgress, Tab::Warframe, Tab::Primary, Tab::Secondary, Tab::Melee, Tab::Archwing, Tab::Companion, Tab::Misc];
    fn label(self) -> &'static str {
        match self {
            Tab::All => "All",
            Tab::InProgress => "In Progress",
            Tab::Warframe => "Warframe",
            Tab::Primary => "Primary",
            Tab::Secondary => "Secondary",
            Tab::Melee => "Melee",
            Tab::Archwing => "Archwing",
            Tab::Companion => "Companion",
            Tab::Misc => "Misc",
        }
    }
    fn matches(self, kind: ItemKind) -> bool {
        match self {
            Tab::All | Tab::InProgress => true,
            Tab::Warframe => kind == ItemKind::Warframe,
            Tab::Primary => kind == ItemKind::Primary,
            Tab::Secondary => kind == ItemKind::Secondary,
            Tab::Melee => kind == ItemKind::Melee,
            Tab::Archwing => matches!(kind, ItemKind::Archwing | ItemKind::ArchGun | ItemKind::ArchMelee | ItemKind::Necramech),
            Tab::Companion => matches!(kind, ItemKind::Companion | ItemKind::CompanionWeapon),
            Tab::Misc => !kind.is_equipment(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Claim {
    #[default]
    All,
    Buildable,
    Missing,
}

#[derive(Default)]
pub struct FoundryView {
    tab: Tab,
    search: String,
    claim: Claim,
    unmastered_only: bool,
    prime: Option<bool>,
}

struct Card<'a> {
    row: &'a FoundryRow,
    result: String,
    kind: ItemKind,
    owned: u64,
    mastered: bool,
}

impl FoundryView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState) {
        if need_inventory(ui, state) {
            return;
        }
        let (Some(inv), Some(db)) = (state.inventory.clone(), state.db.clone()) else { return };
        let pending = foundry::pending(&inv, &db);
        let rows = foundry::rows(&inv, &db);
        let counts = inv.count_map();

        kit::page_title(ui, "Foundry", None);
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            kit::search(ui, &mut self.search, "Search...", 220.0);
            kit::labelled(ui, "Prime", |ui| {
                egui::ComboBox::from_id_salt("prime")
                    .width(100.0)
                    .selected_text(match self.prime {
                        None => "All",
                        Some(true) => "Prime",
                        Some(false) => "Non-prime",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.prime, None, "All");
                        ui.selectable_value(&mut self.prime, Some(true), "Prime");
                        ui.selectable_value(&mut self.prime, Some(false), "Non-prime");
                    });
            });
            kit::labelled(ui, "Claim", |ui| {
                egui::ComboBox::from_id_salt("claim")
                    .width(130.0)
                    .selected_text(match self.claim {
                        Claim::All => "All",
                        Claim::Buildable => "Buildable now",
                        Claim::Missing => "Missing parts",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.claim, Claim::All, "All");
                        ui.selectable_value(&mut self.claim, Claim::Buildable, "Buildable now");
                        ui.selectable_value(&mut self.claim, Claim::Missing, "Missing parts");
                    });
            });
            kit::labelled(ui, "Mastery", |ui| {
                if kit::chip(ui, self.unmastered_only, "Not mastered").clicked() {
                    self.unmastered_only = !self.unmastered_only;
                }
            });
            if kit::ghost_button(ui, "Reset").clicked() {
                *self = FoundryView::default();
            }
        });
        ui.add_space(6.0);
        let tabs: Vec<_> = Tab::ALL.iter().map(|t| (*t, t.label().to_string())).collect();
        kit::tabs(ui, &mut self.tab, &tabs);

        if self.tab == Tab::InProgress {
            if pending.is_empty() {
                kit::empty(ui, "Nothing building", "Builds started in the Foundry appear here.");
            }
            kit::card_grid(ui, 300.0, 64.0, pending.len(), |ui, i| {
                let p = &pending[i];
                let ready = p.completes_at.is_some_and(|t| t <= Utc::now());
                kit::grid_card(ui, ready.then_some(alpha(ACCENT(), 0.6)), |ui| {
                    egui::Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
                        kit::card_title(ui, &p.name);
                        if ready {
                            ui.label(RichText::new("READY TO CLAIM").size(11.0).color(ACCENT()).family(theme::body_strong()));
                        } else {
                            ui.label(RichText::new(format!("Done in {}", format::until(p.completes_at))).size(12.0).color(TEXT_MUTED()));
                        }
                    });
                });
            });
            return;
        }

        let q = self.search.trim().to_lowercase();
        let cards: Vec<Card> = rows
            .iter()
            .filter_map(|row| {
                let recipe = db.recipes.get(&row.recipe)?;
                let info = db.get(&recipe.result_type);
                let kind = info.map(|i| i.kind).unwrap_or(ItemKind::Other);
                let xp = inv.xp_info.get(&recipe.result_type).copied().unwrap_or(0);
                let max = info.map(|i| i.max_level).unwrap_or(30);
                let mastered = info.is_some_and(|i| i.masterable) && level_from_xp(xp, kind.is_frame_like(), max) >= max;
                Some(Card {
                    row,
                    result: recipe.result_type.clone(),
                    kind,
                    owned: counts.get(recipe.result_type.as_str()).copied().unwrap_or(0),
                    mastered,
                })
            })
            .filter(|c| self.tab.matches(c.kind))
            .filter(|c| q.is_empty() || c.row.result_name.to_lowercase().contains(&q))
            .filter(|c| match self.claim {
                Claim::All => true,
                Claim::Buildable => c.row.buildable > 0,
                Claim::Missing => c.row.buildable == 0,
            })
            .filter(|c| !self.unmastered_only || !c.mastered)
            .filter(|c| self.prime.is_none_or(|p| p == c.row.result_name.contains("Prime")))
            .collect();
        if cards.is_empty() {
            kit::empty(ui, "No blueprints", "No owned blueprints match these filters.");
            return;
        }

        let mut open = None;
        kit::card_grid(ui, 340.0, 196.0, cards.len(), |ui, i| {
            let c = &cards[i];
            let ready = c.row.buildable > 0;
            let resp = kit::grid_card(ui, None, |ui| {
                egui::Frame::new().inner_margin(Margin::symmetric(14, 12)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        kit::remote_image(ui, db.image_url(&c.result).as_deref(), vec2(46.0, 46.0));
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(RichText::new(&c.row.result_name).family(theme::body_strong()).size(15.0)).truncate());
                            let (text, color) = if ready { ("READY", ACCENT()) } else { ("MISSING PARTS", TEXT_MUTED()) };
                            ui.label(RichText::new(text).size(11.0).color(color).family(theme::body_strong()));
                        });
                    });
                    ui.add_space(6.0);
                    egui::Grid::new(("ing", i)).num_columns(2).spacing([24.0, 8.0]).show(ui, |ui| {
                        for (n, ing) in c.row.ingredients.iter().take(4).enumerate() {
                            ui.horizontal(|ui| {
                                kit::remote_image(ui, db.image_url(&ing.unique_name).as_deref(), vec2(26.0, 26.0)).on_hover_text(&ing.name);
                                kit::have_need(ui, ing.have, ing.need);
                            });
                            if n % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{} cr", kit::compact_num(c.row.build_price))).color(ACCENT()).family(theme::body_strong()));
                            if c.row.owned_blueprints > 1 {
                                ui.label(RichText::new(format!("×{} bp", c.row.owned_blueprints)).size(11.5).color(TEXT_MUTED()));
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if c.kind.is_equipment() {
                                    if c.mastered {
                                        kit::pill(ui, "Mastered", POSITIVE());
                                    } else {
                                        kit::pill(ui, "Not mastered", TEXT_SECONDARY());
                                    }
                                }
                                kit::pill(ui, &format!("{} owned", c.owned), TEXT_SECONDARY());
                            });
                        });
                    });
                });
            });
            if resp.clicked() {
                open = Some(c.result.clone());
            }
        });
        if let Some(u) = open {
            state.open_item(u);
        }
    }
}
