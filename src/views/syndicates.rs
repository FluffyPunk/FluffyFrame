//! Syndicates (original: `src/views/SyndicatesView.svelte`, `src/lib/syndicates/*`).

use egui::{Align, Layout, RichText, Ui};
use wfhelper_core::config::game_tables;
use wfhelper_core::services::inventory::Affiliation;

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

fn max_rank(tag: &str) -> i64 {
    match tag {
        "NecraloidSyndicate" => 3,
        "QuillsSyndicate" | "VoxSyndicate" => 4,
        _ => 5,
    }
}

#[derive(Default)]
pub struct SyndicatesView;


fn syndicate_card(ui: &mut Ui, a: &Affiliation, daily: Option<i64>) {
    kit::card(ui, |ui| {
        ui.set_width(ui.available_width());
        let name = game_tables::syndicate_label(&a.tag);
        let top = max_rank(&a.tag);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&name).family(theme::body_strong()).size(15.0));
            ui.label(RichText::new(format!("Rank {}", a.title)).size(12.5).color(TEXT_SECONDARY()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::wiki_button(ui, &name);
                if let Some(d) = daily {
                    ui.label(RichText::new(format!("{} standing left today", format::compact(d.max(0) as u64))).size(12.0).color(TEXT_MUTED()));
                }
            });
        });
        let (frac, note) = progress(a.title, a.standing, top);
        ui.horizontal(|ui| {
            let w = ui.available_width() - 190.0;
            ui.allocate_ui(egui::vec2(w, 10.0), |ui| {
                ui.set_width(w);
                kit::progress(ui, frac, if a.title < 0 { NEGATIVE() } else { ACCENT() }, 4.0);
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(note).size(12.0).color(TEXT_MUTED()));
            });
        });
    });
    ui.add_space(8.0);
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
        ui.add_space(8.0);
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
        if aligned.is_empty() && open.is_empty() && other.is_empty() {
            kit::empty(ui, "No syndicates", "The inventory lists no syndicate standing.");
            return;
        }
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            for (title, list, daily) in [
                ("Aligned syndicates", &aligned, inv.daily_standing),
                ("Open worlds", &open, None),
                ("Other", &other, None),
            ] {
                if list.is_empty() {
                    continue;
                }
                kit::caption(ui, title);
                ui.add_space(4.0);
                for a in list.iter() {
                    syndicate_card(ui, a, daily);
                }
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
}
