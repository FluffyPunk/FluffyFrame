//! Syndicates (original: `src/views/SyndicatesView.svelte`, `src/lib/syndicates/*`).

use egui::{FontId, RichText, Ui, vec2};
use fluffyframe_core::config::game_tables;
use fluffyframe_core::services::inventory::Affiliation;

use super::need_inventory;
use crate::components::{format, ui as kit, widgets};
use crate::stores::AppState;
use crate::theme::{self, *};

const ALIGNED: &[&str] = &[
    "ArbitersSyndicate",
    "CephalonSudaSyndicate",
    "NewLokaSyndicate",
    "PerrinSyndicate",
    "RedVeilSyndicate",
    "SteelMeridianSyndicate",
];
const OPEN_WORLDS: &[&str] = &[
    "CetusSyndicate",
    "QuillsSyndicate",
    "SolarisSyndicate",
    "VentKidsSyndicate",
    "VoxSyndicate",
    "EntratiSyndicate",
    "NecraloidSyndicate",
    "ZarimanSyndicate",
    "EntratiLabSyndicate",
    "HexSyndicate",
    "KahlSyndicate",
];

/// Standing span of each rank (index = rank). Standing is cumulative: rank 5
/// spans 240,000 → 372,000.
const RANK_SPAN: [i64; 6] = [5_000, 22_000, 44_000, 70_000, 99_000, 132_000];
/// Upper bounds of the negative ranks: rank -1 ends at -5,000, rank -2 at -27,000.
const NEG_FLOOR: [i64; 3] = [-5_000, -27_000, -71_000];

/// Cumulative standing at which `rank` (>= 1) starts.
fn rank_start(rank: i64) -> i64 {
    RANK_SPAN[..rank.clamp(0, 6) as usize].iter().sum()
}

/// (bar fraction, note) for a syndicate's rank and cumulative standing.
fn progress(title: i64, standing: i64, top: i64) -> (f32, String) {
    if title >= 1 {
        let start = rank_start(title);
        let span = RANK_SPAN[(title as usize).min(5)];
        let frac = (standing - start) as f32 / span as f32;
        if title >= top {
            (frac, "Highest rank reached".into())
        } else {
            let need = (rank_start(title + 1) - standing).max(0) as u64;
            (frac, format!("{} standing to rank {}", format::compact(need), title + 1))
        }
    } else if title == 0 {
        let frac = (standing + 5_000) as f32 / 10_000.0;
        (frac, format!("{} standing to rank 1", format::compact((5_000 - standing).max(0) as u64)))
    } else {
        // Negative ranks: the bar fills toward the next rank up.
        let idx = (-title) as usize; // 1 or 2
        let upper = NEG_FLOOR[idx - 1];
        let lower = NEG_FLOOR[idx.min(2)];
        let frac = if lower == upper { 0.0 } else { (standing - lower) as f32 / (upper - lower) as f32 };
        (frac, format!("{} standing to rank {}", format::compact((upper - standing).max(0) as u64), title + 1))
    }
}

/// What the game shows as standing: (earned in the current rank, that rank's
/// cap). `None` for negative ranks, shown as the signed total. Syndicates whose
/// standing isn't the cumulative ladder (Kahl's Garrison) keep their raw value.
fn rank_window(title: i64, standing: i64) -> Option<(i64, i64)> {
    match title {
        t if t >= 1 => {
            let span = RANK_SPAN[(t as usize).min(5)];
            let earned = standing - rank_start(t);
            Some(if earned < 0 { (standing, span) } else { (earned, span) })
        }
        0 => Some((standing, 5_000)),
        _ => None,
    }
}

/// Cephalon Simaris has no ranks: his standing is a balance (always rank 0)
/// capped at 125,000.
const SIMARIS_CAP: i64 = 125_000;

fn is_simaris(tag: &str) -> bool {
    matches!(tag, "LibrarySyndicate" | "CephalonSimarisSyndicate")
}

fn max_rank(tag: &str) -> i64 {
    match tag {
        "NecraloidSyndicate" => 3,
        "QuillsSyndicate" | "VoxSyndicate" => 4,
        _ => 5,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Aligned,
    OpenWorlds,
    Other,
}

#[derive(Default)]
pub struct SyndicatesView {
    tab: Tab,
}

/// Card width the grid aims for: as many as fit, up to four per row.
const CARD_MIN_WIDTH: f32 = 300.0;
const EMBLEM: f32 = 56.0;

/// The syndicate's sigil, or its initial when it has none (or the image is
/// still loading from DE's CDN).
fn emblem(ui: &mut Ui, url: Option<&str>, name: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(EMBLEM, EMBLEM), egui::Sense::hover());
    ui.painter().rect_filled(rect, RADIUS_MD(), BG_WELL());
    match url {
        Some(url) => kit::paint_image_fit(ui, url, rect.shrink(4.0)),
        None => {
            let initial = name.trim_start_matches("The ").chars().next().unwrap_or('?').to_string();
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initial, FontId::new(26.0, theme::display_bold()), TEXT_MUTED());
        }
    }
}

fn standing_text(standing: i64) -> String {
    let n = format::int(standing.unsigned_abs());
    if standing < 0 { format!("−{n}") } else { n }
}

fn syndicate_card(ui: &mut Ui, a: &Affiliation, emblem_url: Option<String>) {
    let name = game_tables::syndicate_label(&a.tag);
    let top = max_rank(&a.tag);
    let simaris = is_simaris(&a.tag);
    let (frac, note, window) = if simaris {
        let left = (SIMARIS_CAP - a.standing).max(0) as u64;
        let note = if left == 0 { "Standing cap reached".to_string() } else { format!("{} standing to the cap", format::compact(left)) };
        (a.standing as f32 / SIMARIS_CAP as f32, note, Some((a.standing, SIMARIS_CAP)))
    } else {
        let (frac, note) = progress(a.title, a.standing, top);
        (frac, note, rank_window(a.title, a.standing))
    };
    let (rank_color, rank) = if simaris {
        (TEXT_MUTED(), "No ranks".to_string())
    } else if a.title >= top {
        (POSITIVE(), format!("Rank {} · Max", a.title))
    } else if a.title < 0 {
        (NEGATIVE(), format!("Rank {}", a.title))
    } else {
        (ACCENT(), format!("Rank {}", a.title))
    };
    kit::card(ui, |ui| {
        ui.set_width(ui.available_width());
        kit::split_row(
            ui,
            |ui| {
                emblem(ui, emblem_url.as_deref(), &name);
                ui.add_space(4.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&name).font(FontId::new(18.0, theme::display_bold())));
                    kit::pill(ui, &rank, rank_color);
                });
            },
            |ui| widgets::wiki_button(ui, &name),
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let value = FontId::new(22.0, theme::display_bold());
            match window {
                Some((earned, cap)) => {
                    ui.label(RichText::new(standing_text(earned)).font(value).color(if earned < 0 { NEGATIVE() } else { TEXT() }));
                    ui.label(RichText::new(format!("/ {}", format::int(cap as u64))).font(FontId::new(15.0, theme::display_bold())).color(TEXT_SECONDARY()));
                }
                None => {
                    ui.label(RichText::new(standing_text(a.standing)).font(value).color(NEGATIVE()));
                }
            }
            ui.label(RichText::new("standing").size(12.0).color(TEXT_MUTED()));
        });
        kit::progress(ui, frac.clamp(0.0, 1.0), if a.title < 0 { NEGATIVE() } else { ACCENT() }, 6.0);
        ui.add_space(2.0);
        ui.label(RichText::new(note).size(12.0).color(TEXT_MUTED()));
    });
}

impl SyndicatesView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState) {
        if need_inventory(ui, state) {
            return;
        }
        let Some(inv) = state.inventory.clone() else { return };
        ui.horizontal(|ui| {
            kit::page_title(ui, "Syndicates", None);
            kit::page_subtitle(ui, "Standing and rank for every syndicate on your account.");
        });
        let mut aligned: Vec<_> = inv.affiliations.iter().filter(|a| ALIGNED.contains(&a.tag.as_str())).collect();
        let mut open: Vec<_> = inv.affiliations.iter().filter(|a| OPEN_WORLDS.contains(&a.tag.as_str())).collect();
        let mut other: Vec<_> = inv
            .affiliations
            .iter()
            .filter(|a| !ALIGNED.contains(&a.tag.as_str()) && !OPEN_WORLDS.contains(&a.tag.as_str()) && !a.tag.starts_with("RadioLegion"))
            .collect();
        for list in [&mut aligned, &mut open, &mut other] {
            list.sort_by_key(|a| game_tables::syndicate_label(&a.tag));
        }
        let tabs: Vec<(Tab, String)> = [(Tab::Aligned, "Aligned", &aligned), (Tab::OpenWorlds, "Open worlds", &open), (Tab::Other, "Other", &other)]
            .into_iter()
            .filter(|(_, _, list)| !list.is_empty())
            .map(|(tab, label, list)| (tab, format!("{label} ({})", list.len())))
            .collect();
        let Some(first) = tabs.first().map(|(t, _)| *t) else {
            kit::empty(ui, "No syndicates", "The inventory lists no syndicate standing.");
            return;
        };
        if !tabs.iter().any(|(t, _)| *t == self.tab) {
            self.tab = first;
        }
        kit::tabs(ui, &mut self.tab, &tabs);
        let list = match self.tab {
            Tab::Aligned => &aligned,
            Tab::OpenWorlds => &open,
            Tab::Other => &other,
        };
        let db = state.db.clone();

        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            // The daily cap is shared by all aligned syndicates, not per card.
            if self.tab == Tab::Aligned
                && let Some(d) = inv.daily_standing
            {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format::int(d.max(0) as u64)).font(FontId::new(17.0, theme::display_bold())).color(ACCENT_BRIGHT()));
                    ui.label(RichText::new("standing left today, shared by all aligned syndicates").size(12.0).color(TEXT_MUTED()));
                });
                ui.add_space(6.0);
            }
            let per_row = kit::balanced(kit::fit_columns(ui, CARD_MIN_WIDTH, 4), list.len());
            for row in list.chunks(per_row) {
                ui.columns(per_row, |cols| {
                    for (col, a) in cols.iter_mut().zip(row) {
                        let url = db.as_ref().and_then(|db| db.syndicate_emblem_url(&a.tag, a.title));
                        syndicate_card(col, a, url);
                    }
                });
                ui.add_space(8.0);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_original_bars() {
        // Values from a real account, checked against the original app's screen.
        assert!((super::progress(5, 249_780, 5).0 - 0.074).abs() < 0.001);
        assert!((super::progress(5, 372_000, 5).0 - 1.0).abs() < 0.001);
        assert_eq!(super::progress(-2, -71_000, 5).1, "44.0k standing to rank -1");
        assert_eq!(super::progress(1, 27_000, 3).1, "0 standing to rank 2");
    }

    #[test]
    fn standing_within_rank() {
        // Rank 5 starts at 240,000 and spans 132,000.
        assert_eq!(super::rank_window(5, 249_780), Some((9_780, 132_000)));
        assert_eq!(super::rank_window(5, 372_000), Some((132_000, 132_000)));
        // Rank 2 starts at 27,000 and spans 44,000.
        assert_eq!(super::rank_window(2, 50_408), Some((23_408, 44_000)));
        assert_eq!(super::rank_window(0, 1_200), Some((1_200, 5_000)));
        assert_eq!(super::rank_window(-2, -71_000), None);
        // Not on the cumulative ladder (Kahl's Garrison): the raw value.
        assert_eq!(super::rank_window(5, 5), Some((5, 132_000)));
    }
}
