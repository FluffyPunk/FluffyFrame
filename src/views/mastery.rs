//! Mastery Helper (original: `src/views/MasteryView.svelte`, `src/components/mastery/*`,
//! `src/lib/mastery*.ts`).

use egui::{Align, FontId, Layout, Margin, RichText, Ui, vec2};
use fluffyframe_core::services::item_database::ItemKind;
use fluffyframe_core::services::mastery_helper::{
    self, ChartProgress, MasteryRow, Progress, Source, points_per_rank, potential, total_xp,
};

use super::need_inventory;
use crate::assets;
use crate::components::{format, ui as kit};
use crate::stores::AppState;
use crate::theme::{self, *};
use crate::i18n::{tr, trf, trn, tkey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Roadmap,
    Collection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Status {
    #[default]
    All,
    Mastered,
    InProgress,
    Missing,
}


#[derive(Default)]
pub struct MasteryView {
    tab: Tab,
    search: String,
    group: Option<&'static str>,
    status: Status,
    route: Source,
}

fn remaining_xp(r: &MasteryRow) -> u64 {
    (r.max_level.saturating_sub(r.level)) as u64 * points_per_rank(r.kind)
}

impl MasteryView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState) {
        if need_inventory(ui, state) {
            return;
        }
        let (Some(inv), Some(db)) = (state.inventory.clone(), state.db.clone()) else { return };
        let (rows, summary) = mastery_helper::rows(&inv, &db);

        kit::page_title(ui, tr("Mastery Helper"), None);
        kit::tabs(ui, &mut self.tab, &[(Tab::Collection, tr(tr("Collection")).into()), (Tab::Roadmap, tr(tr("MR Roadmap")).into())]);

        let in_progress = rows.iter().filter(|r| !r.mastered && r.level > 0).count();
        let missing = rows.iter().filter(|r| !r.mastered && r.level == 0).count();
        // Unmastered items with a known route, best route first, then most XP.
        let roadmap: Vec<&MasteryRow> = {
            let mut v: Vec<_> = rows.iter().filter(|r| !r.mastered && r.source != Source::Other).collect();
            v.sort_by(|a, b| a.source.cmp(&b.source).then(remaining_xp(b).cmp(&remaining_xp(a))).then(a.name.cmp(&b.name)));
            v
        };
        let easy: Vec<&MasteryRow> = roadmap.iter().copied().filter(|r| r.source <= Source::CraftIt).collect();

        // Rank reached by doing all the easy mastery: level, claim and craft what's available now.
        let easy_xp: u64 = easy.iter().map(|r| remaining_xp(r)).sum();
        let pot = potential(total_xp(&inv, &db, summary.points), easy_xp);
        let mr_title = trf("MR {} → MR {}", &[&inv.mastery_rank, &pot.rank]);
        let mr_sub = trf("{}/{} XP", &[&format::int(pot.into), &format::int(pot.span)]);

        let mut open: Option<String> = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            // The ring + one-row summary needs about 1050px; narrower windows
            // get the same numbers in a wrapping card.
            let pct = summary.mastered as f32 / summary.total.max(1) as f32;
            if ui.available_width() < 1050.0 {
                kit::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    // A grid, as many per row as fit (a wrapping row steps each
                    // stat a little lower), then MR on its own line.
                    let stats = [
                        (format!("{:.1}%", pct * 100.0), tr("Complete"), INFO()),
                        (summary.mastered.to_string(), tr("Mastered"), POSITIVE()),
                        (in_progress.to_string(), tr("In Progress"), WARNING()),
                        (missing.to_string(), tr("Missing"), NEGATIVE()),
                        (summary.total.to_string(), tr("Total"), TEXT()),
                    ];
                    let per_row = kit::balanced(kit::fit_columns(ui, 200.0, stats.len()), stats.len());
                    for row in stats.chunks(per_row) {
                        ui.columns(per_row, |cols| {
                            for (ui, (value, label, color)) in cols.iter_mut().zip(row) {
                                kit::big_stat(ui, value, label, *color);
                            }
                        });
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&mr_title).font(FontId::new(40.0, theme::display_bold())));
                        ui.label(RichText::new(&mr_sub).color(TEXT_SECONDARY()));
                    });
                });
            } else {
            ui.horizontal(|ui| {
                kit::ring(ui, pct, 118.0, INFO(), tr("Mastered"));
                ui.add_space(8.0);
                kit::card(ui, |ui| {
                    // As tall as the ring beside it (118 minus the card's 14px
                    // margins), stats centered vertically: a frame can't be
                    // centered in a row, so this sizes it instead.
                    let mr_line = ui.fonts_mut(|f| f.row_height(&FontId::new(40.0, theme::display_bold())));
                    let body_line = ui.text_style_height(&egui::TextStyle::Body);
                    let stats_h: f32 = mr_line + ui.spacing().item_spacing.y + body_line;
                    const INNER: f32 = 118.0 - 2.0 * 14.0 - 2.0;
                    ui.vertical(|ui| {
                        ui.set_min_height(INNER);
                        ui.add_space((INNER - stats_h) / 2.0);
                        // Top-aligned: a centered `horizontal` row steps each
                        // stat a little lower than the one before as it grows.
                        // The stats share one font, so their tops line up.
                        ui.horizontal_top(|ui| {
                        // Separators stretch to the available height.
                        ui.set_max_height(stats_h);
                        // The one-line stats, centered on the two-line MR block.
                        let big = ui.fonts_mut(|f| f.row_height(&FontId::new(40.0, theme::display_bold())));
                        let pad = ((stats_h - big) / 2.0).max(0.0);
                        let stat = |ui: &mut Ui, value: &str, label: &str, color| {
                            ui.vertical(|ui| {
                                ui.add_space(pad);
                                kit::big_stat(ui, value, label, color);
                            });
                        };
                        ui.add_space(12.0);
                        stat(ui, &summary.mastered.to_string(), tr("Mastered"), POSITIVE());
                        ui.separator();
                        stat(ui, &in_progress.to_string(), tr("In Progress"), WARNING());
                        ui.separator();
                        stat(ui, &missing.to_string(), tr("Missing"), NEGATIVE());
                        ui.separator();
                        stat(ui, &summary.total.to_string(), tr("Total"), TEXT());
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&mr_title).font(FontId::new(40.0, theme::display_bold())));
                            ui.label(RichText::new(&mr_sub).color(TEXT_SECONDARY()));
                        });
                        ui.add_space(12.0);
                        });
                    });
                });
            });
            }
            ui.add_space(10.0);

            let owned_xp: u64 = easy.iter().filter(|r| r.source == Source::Owned).map(|r| remaining_xp(r)).sum();
            let bp_xp = easy_xp - owned_xp;
            ui.columns(3, |cols| {
                insight(&mut cols[0], tr("Easy mastery"), &trf("+{} XP", &[&format::int(easy_xp)]), POSITIVE(), &trn("{} item owned or buildable now", "{} items owned or buildable now", (easy.len()) as u64, &[&easy.len()]));
                insight(&mut cols[1], tr("Level what you own"), &trf("+{} XP", &[&format::int(owned_xp)]), ACCENT(), tr("Owned and not yet rank 30"));
                insight(&mut cols[2], tr("From the Foundry"), &trf("+{} XP", &[&format::int(bp_xp)]), INFO(), tr("Ready to claim, or craftable right now"));
            });
            ui.add_space(10.0);

            match self.tab {
                Tab::Roadmap => {
                    const ROUTES: [Source; 6] =
                        [Source::Owned, Source::ReadyToClaim, Source::CraftIt, Source::CraftParts, Source::Market, Source::ClanLab];
                    let tabs: Vec<(Source, String)> = ROUTES
                        .iter()
                        .map(|&s| (s, format!("{} ({})", tr(s.label()), roadmap.iter().filter(|r| r.source == s).count())))
                        .collect();
                    kit::tabs(ui, &mut self.route, &tabs);
                    let shown: Vec<&&MasteryRow> = roadmap.iter().filter(|r| r.source == self.route).collect();
                    ui.add_space(4.0);
                    // As many cards per row as fit at a readable minimum width, at most 4.
                    let gap = 8.0;
                    let avail = ui.available_width();
                    let cols = (((avail + gap) / (300.0 + gap)).floor() as usize).clamp(1, 4);
                    let w = (avail - gap * (cols - 1) as f32) / cols as f32;
                    for chunk in shown.chunks(cols) {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for r in chunk {
                                ui.allocate_ui(vec2(w, 0.0), |ui| {
                                    ui.set_width(w);
                                    if recommendation(ui, r, db.image_url(&r.unique_name).as_deref()).clicked() {
                                        open = Some(r.unique_name.clone());
                                    }
                                });
                            }
                        });
                        ui.add_space(4.0);
                    }
                    if shown.is_empty() {
                        kit::empty(ui, tr("Nothing here"), tr("No unmastered items on this route."));
                    }
                }
                Tab::Collection => self.collection(ui, &rows, &mastery_helper::chart_progress(&inv, &db), &db, &mut open),
            }
        });
        if let Some(u) = open {
            state.open_item(u);
        }
    }

    fn collection(
        &mut self,
        ui: &mut Ui,
        rows: &[MasteryRow],
        chart: &ChartProgress,
        db: &fluffyframe_core::services::item_database::ItemDb,
        open: &mut Option<String>,
    ) {
        egui::CollapsingHeader::new(RichText::new(tr("Detailed breakdown")).font(FontId::new(15.0, theme::display_bold())))
            .default_open(true)
            .show(ui, |ui| {
                kit::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = ROW_GAP;
                    for (label, kinds) in GROUPS {
                        let in_group: Vec<_> = rows.iter().filter(|r| kinds.contains(&r.kind)).collect();
                        if in_group.is_empty() {
                            continue;
                        }
                        let done = in_group.iter().filter(|r| r.mastered).count() as u64;
                        let started = in_group.iter().filter(|r| r.mastered || r.level > 0).count() as u64;
                        breakdown_row(ui, tr(label), Progress { done, total: in_group.len() as u64 }, started, POSITIVE());
                    }
                });
                ui.add_space(8.0);
                // Star chart and intrinsics side by side, stacked when narrow.
                kit::responsive_columns(ui, 420.0, 2, |ui, col| {
                    kit::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let (title, rows, color) = if col == 0 {
                            let rows = [
                                (tr("Normal"), chart.nodes),
                                (tr("Junctions"), chart.junctions),
                                (tr("Steel Path"), chart.steel_path),
                                (tr("Steel P. Junctions"), chart.steel_junctions),
                            ];
                            (tr("Star chart"), rows.to_vec(), INFO())
                        } else {
                            (tr("Intrinsics"), vec![(tr("Railjack"), chart.railjack), (tr("Drifter"), chart.drifter)], ACCENT())
                        };
                        ui.label(RichText::new(title).font(FontId::new(15.0, theme::display_bold())));
                        ui.add_space(2.0);
                        ui.spacing_mut().item_spacing.y = ROW_GAP;
                        for (label, p) in rows {
                            breakdown_row(ui, label, p, p.done, color);
                        }
                    });
                });
            });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            kit::search(ui, &mut self.search, tr("Search..."), 240.0);
        });
        ui.add_space(4.0);
        let mut cat: Option<&str> = self.group;
        let tabs: Vec<(Option<&str>, String)> =
            std::iter::once((None, tr("All").to_string())).chain(GROUPS.iter().map(|(l, _)| (Some(*l), tr(l).to_string()))).collect();
        if kit::tabs(ui, &mut cat, &tabs) {
            self.group = cat;
        }
        let status_tabs = [
            (Status::All, tr("All").to_string()),
            (Status::Missing, tr("Missing").to_string()),
            (Status::InProgress, tr("In Progress").to_string()),
            (Status::Mastered, tr("Mastered").to_string()),
        ];
        kit::tabs(ui, &mut self.status, &status_tabs);
        let q = self.search.trim().to_lowercase();
        let shown: Vec<&MasteryRow> = rows
            .iter()
            .filter(|r| self.group.is_none_or(|g| GROUPS.iter().any(|(l, k)| *l == g && k.contains(&r.kind))))
            .filter(|r| q.is_empty() || r.name.to_lowercase().contains(&q))
            .filter(|r| match self.status {
                Status::All => true,
                Status::Mastered => r.mastered,
                Status::InProgress => !r.mastered && r.level > 0,
                Status::Missing => !r.mastered && r.level == 0,
            })
            .collect();
        // A plain grid (this tab sits inside the page scroll area).
        let gap = 10.0;
        let avail = ui.available_width();
        let cols = ((avail + gap) / (190.0 + gap)).floor().max(1.0) as usize;
        let w = (avail - gap * (cols - 1) as f32) / cols as f32;
        for chunk in shown.chunks(cols) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for r in chunk {
                    ui.allocate_ui(vec2(w, 196.0), |ui| {
                        ui.set_width(w);
                        ui.set_height(196.0);
                        let border = if r.mastered { Some(alpha(POSITIVE(), 0.5)) } else { None };
                        let resp = kit::grid_card(ui, border, |ui| {
                            let width = ui.available_width();
                            let (well, _) = ui.allocate_exact_size(vec2(width, 110.0), egui::Sense::hover());
                            ui.painter().rect_filled(well, egui::CornerRadius { nw: RADIUS_LG(), ne: RADIUS_LG(), sw: 0, se: 0 }, BG_WELL());
                            if let Some(url) = db.image_url(&r.unique_name) {
                                kit::paint_image_fit(ui, &url, egui::Rect::from_center_size(well.center(), vec2(86.0, 86.0)));
                            }
                            if r.mastered {
                                let c = well.left_top() + vec2(16.0, 16.0);
                                ui.painter().circle(c, 8.0, alpha(POSITIVE(), 0.15), egui::Stroke::new(1.0, POSITIVE()));
                                ui.painter().text(c, egui::Align2::CENTER_CENTER, "M", FontId::new(9.5, theme::body_strong()), POSITIVE());
                            }
                            egui::Frame::new().inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
                                ui.add(egui::Label::new(RichText::new(&r.name).family(theme::body_strong())).truncate());
                                ui.label(RichText::new(tr(r.kind.label())).size(11.5).color(TEXT_MUTED()));
                                let color = if r.mastered { POSITIVE() } else if r.level > 0 { WARNING() } else { TEXT_MUTED() };
                                kit::progress(ui, r.level as f32 / r.max_level.max(1) as f32, color, 3.0);
                                ui.label(RichText::new(format!("{}/{}", r.level, r.max_level)).size(11.0).color(TEXT_MUTED()));
                            });
                        });
                        if resp.clicked() {
                            *open = Some(r.unique_name.clone());
                        }
                    });
                }
            });
            ui.add_space(gap - ui.spacing().item_spacing.y);
        }
    }
}

const GROUPS: [(&str, &[ItemKind]); 9] = [
    (tkey("Warframes"), &[ItemKind::Warframe]),
    (tkey("Primary"), &[ItemKind::Primary]),
    (tkey("Secondary"), &[ItemKind::Secondary]),
    (tkey("Melee"), &[ItemKind::Melee]),
    (tkey("Companions"), &[ItemKind::Companion, ItemKind::CompanionWeapon]),
    (tkey("Archwing"), &[ItemKind::Archwing, ItemKind::ArchGun, ItemKind::ArchMelee]),
    (tkey("Amps"), &[ItemKind::Amp]),
    (tkey("Necramech"), &[ItemKind::Necramech]),
    (tkey("Misc"), &[ItemKind::KDrive]),
];

/// Vertical gap between breakdown rows.
const ROW_GAP: f32 = 4.0;

/// Label, "done/total (pct)", then a bar: `done` in `color`, `started` (≥ done) dimmed behind it.
fn breakdown_row(ui: &mut Ui, label: &str, p: Progress, started: u64, color: egui::Color32) {
    let total = p.total.max(1) as f32;
    // Rows default to a control's height; these are plain text and a 4px bar.
    ui.spacing_mut().interact_size.y = 14.0;
    ui.horizontal(|ui| {
        ui.allocate_ui(vec2(140.0, 14.0), |ui| {
            ui.set_width(140.0);
            ui.label(RichText::new(label).size(11.5).color(TEXT_SECONDARY()));
        });
        ui.allocate_ui(vec2(110.0, 14.0), |ui| {
            ui.set_width(110.0);
            ui.label(RichText::new(format!("{}/{} ({:.1}%)", p.done, p.total, p.done as f32 / total * 100.0)).size(10.5).color(TEXT_MUTED()));
        });
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 4.0), egui::Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 2.0, BG_HOVER());
        let mut a = rect;
        a.set_width(rect.width() * started as f32 / total);
        painter.rect_filled(a, 2.0, ACCENT_DIM());
        let mut m = rect;
        m.set_width(rect.width() * p.done as f32 / total);
        painter.rect_filled(m, 2.0, color);
    });
}

fn insight(ui: &mut Ui, title: &str, value: &str, color: egui::Color32, sub: &str) {
    kit::card(ui, |ui| {
        ui.set_width(ui.available_width());
        kit::caption(ui, title);
        ui.label(RichText::new(value).font(FontId::new(24.0, theme::display_bold())).color(color));
        ui.label(RichText::new(sub).size(11.5).color(TEXT_MUTED()));
    });
}

fn recommendation(ui: &mut Ui, r: &MasteryRow, image: Option<&str>) -> egui::Response {
    let resp = egui::Frame::new()
        .fill(BG_SURFACE())
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG()))
        .corner_radius(RADIUS_LG())
        .inner_margin(Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                egui::Frame::new().fill(BG_WELL()).corner_radius(RADIUS_MD()).show(ui, |ui| {
                    kit::remote_image(ui, image, vec2(52.0, 52.0));
                });
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&r.name).family(theme::body_strong()).size(15.0));
                        if r.mastery_req > 0 {
                            kit::pill(ui, &trf("MR {}", &[&r.mastery_req]), TEXT_SECONDARY());
                        }
                    });
                    ui.label(RichText::new(format!("{} · {}", tr(r.kind.label()), tr(r.source.label()))).size(12.0).color(TEXT_SECONDARY()));
                    let progress = match (r.source, r.parts) {
                        (Source::Owned | Source::ReadyToClaim, _) | (_, None) => trf("Level {}/{}", &[&r.level, &r.max_level]),
                        (_, Some((have, total))) => trf("Parts {}/{}", &[&have, &total]),
                    };
                    ui.label(RichText::new(progress).size(12.0).color(TEXT_MUTED()));
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(trf("+{} XP", &[&(format::int(remaining_xp(r)))])).family(theme::body_strong()).color(ACCENT()));
                });
            });
        })
        .response
        .interact(egui::Sense::click());
    let _ = assets::MASTERY;
    resp
}
