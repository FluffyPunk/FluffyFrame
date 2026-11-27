//! Riven calculator (original: browse.wf's Riven Calculator): every stat a riven
//! type can roll, with its range for a stat setup and rank. Weapons aren't
//! picked: every riven-capable weapon shares one disposition.

use egui::{FontId, RichText, Ui};
use fluffyframe_core::services::rivens::{self, DISPOSITION, MAX_RANK, Setup, StatRange};

use crate::components::ui as kit;
use crate::stores::AppState;
use crate::theme::{self, *};
use crate::i18n::{tr, trf, tkey};

/// Weapon types and their riven mod.
const TYPES: [(&str, &str); 7] = [
    (tkey("Rifle"), "LotusRifleRandomModRare"),
    (tkey("Shotgun"), "LotusShotgunRandomModRare"),
    (tkey("Pistol"), "LotusPistolRandomModRare"),
    (tkey("Melee"), "PlayerMeleeWeaponRandomModRare"),
    (tkey("Archgun"), "LotusArchgunRandomModRare"),
    (tkey("Kitgun"), "LotusModularPistolRandomModRare"),
    (tkey("Zaw"), "LotusModularMeleeRandomModRare"),
];

pub struct RivenCalcView {
    /// Index into [`TYPES`].
    kind: usize,
    setup: Setup,
    rank: u32,
    search: String,
}

impl Default for RivenCalcView {
    fn default() -> Self {
        Self { kind: 0, setup: Setup::default(), rank: MAX_RANK, search: String::new() }
    }
}

impl RivenCalcView {
    pub fn ui(&mut self, ui: &mut Ui, state: &AppState) {
        let Some(db) = state.db.as_ref() else {
            kit::empty(ui, tr("Loading game data"), tr("Riven stats come from DE's export."));
            return;
        };
        ui.horizontal_wrapped(|ui| {
            kit::labelled(ui, tr("Weapon type"), |ui| {
                egui::ComboBox::from_id_salt("calc-type").width(110.0).selected_text(tr(TYPES[self.kind].0)).show_ui(ui, |ui| {
                    for (i, (name, _)) in TYPES.iter().enumerate() {
                        ui.selectable_value(&mut self.kind, i, tr(name));
                    }
                });
            });
            kit::labelled(ui, tr("Setup"), |ui| {
                egui::ComboBox::from_id_salt("calc-setup").width(130.0).selected_text(tr(self.setup.label())).show_ui(ui, |ui| {
                    for s in Setup::ALL {
                        ui.selectable_value(&mut self.setup, s, tr(s.label()));
                    }
                });
            });
            kit::labelled(ui, tr("Rank"), |ui| {
                egui::ComboBox::from_id_salt("calc-rank").width(60.0).selected_text(self.rank.to_string()).show_ui(ui, |ui| {
                    for r in (0..=MAX_RANK).rev() {
                        ui.selectable_value(&mut self.rank, r, r.to_string());
                    }
                });
            });
            kit::search(ui, &mut self.search, tr("Filter stats…"), 180.0);
        });
        ui.add_space(4.0);
        let (buff, curse) = self.setup.factors();
        let mut note = trf("Disposition {} (every weapon) · buffs ×{}", &[&DISPOSITION, &trim(buff)]);
        if let Some(c) = curse {
            note.push_str(&trf(" · curse ×{}", &[&trim(c)]));
        }
        note.push_str(" of base × disposition × 90 at rank 8 · each roll lands within ±10%");
        ui.label(RichText::new(note).size(12.0).color(TEXT_MUTED()));
        ui.add_space(8.0);

        let riven_type = format!("/Lotus/Upgrades/Mods/Randomized/{}", TYPES[self.kind].1);
        let (buffs, curses) = rivens::stat_ranges(&db.rivens, &riven_type, self.setup, self.rank);
        let q = self.search.trim().to_lowercase();
        let keep = |r: &&StatRange| q.is_empty() || r.name.to_lowercase().contains(&q);
        let buffs: Vec<&StatRange> = buffs.iter().filter(keep).collect();
        let curses: Vec<&StatRange> = curses.iter().filter(keep).collect();
        if buffs.is_empty() && curses.is_empty() {
            kit::empty(ui, tr("No stats"), tr("Nothing matches the filter."));
            return;
        }
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let cols = if curses.is_empty() { 1 } else { 2 };
            kit::responsive_columns(ui, 360.0, cols, |ui, col| {
                let (title, list, color) = if col == 0 { (tr("Buffs"), &buffs, POSITIVE()) } else { (tr("Curse"), &curses, NEGATIVE()) };
                kit::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(title).font(FontId::new(16.0, theme::display_bold())).color(color));
                    ui.add_space(2.0);
                    for r in list.iter() {
                        kit::split_row(
                            ui,
                            |ui| {
                                ui.label(RichText::new(&r.name).size(13.0));
                            },
                            |ui| {
                                ui.label(RichText::new(r.text()).size(13.0).family(theme::body_strong()).color(color));
                            },
                        );
                    }
                });
                ui.add_space(8.0);
            });
        });
    }
}

/// 1.2375 → "1.2375", 0.99 → "0.99".
fn trim(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}
