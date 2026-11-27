//! Bounties tab of World: each faction's current board on the left, the selected
//! bounty's drop table on the right.

use std::borrow::Cow;

use chrono::Utc;
use egui::{FontId, Margin, RichText, Ui, vec2};
use fluffyframe_core::services::drop_data::{BountyReward, BountyTable, DropIndex, bounty_source};
use fluffyframe_core::services::item_database::ItemDb;
use fluffyframe_core::services::world_state::WorldState;

use crate::assets;
use crate::components::{format, ui as kit};
use crate::ipc::{Backend, DataCmd};
use crate::stores::AppState;
use crate::theme::{self, *};
use crate::i18n::{tr, trf};

/// The six factions with bounties: tag and name. The first three come with the
/// world state; the others from the seeded bounty cycle.
const FACTIONS: [(&str, &str); 6] = [
    ("CetusSyndicate", "Ostrons"),
    ("SolarisSyndicate", "Solaris United"),
    ("EntratiSyndicate", "Entrati"),
    ("ZarimanSyndicate", "The Holdfasts"),
    ("EntratiLabSyndicate", "Cavia"),
    ("HexSyndicate", "The Hex"),
];

/// Side by side from this width; stacked below it.
const SPLIT_WIDTH: f32 = 860.0;

#[derive(Default)]
pub struct BountiesView {
    /// Index into [`FACTIONS`].
    faction: usize,
    /// Selected row of that faction's board.
    row: usize,
}

/// One bounty on a board.
struct Row<'a> {
    title: Cow<'a, str>,
    /// Challenge name and what it asks, for seeded bounties.
    task: Option<(&'a str, &'a str)>,
    min_level: u32,
    max_level: u32,
    mastery_req: u32,
    rotation: Option<char>,
    ally: Option<&'a str>,
    table: Option<&'a BountyTable>,
}

impl BountiesView {
    pub fn ui(&mut self, ui: &mut Ui, state: &mut AppState, backend: &Backend, ws: &WorldState) {
        if state.drops.is_none() && !state.drops_requested {
            state.drops_requested = true;
            backend.data(DataCmd::LoadDrops);
        }
        let stale = state.bounties.as_ref().is_none_or(|b| b.expiry.is_some_and(|e| e <= Utc::now()));
        if stale && !state.bounties_requested {
            state.bounties_requested = true;
            backend.data(DataCmd::LoadBounties);
        }

        let (tag, _) = FACTIONS[self.faction];
        let live = ws.bounties.iter().find(|b| b.tag == tag);
        let book = state.bounties.clone();
        let seeded = book.as_ref().and_then(|b| b.seeded.get(tag));
        let expiry = live.map_or(book.as_ref().and_then(|b| b.expiry), |b| b.expiry);

        kit::split_row(
            ui,
            |ui| {
                for (i, (tag, name)) in FACTIONS.iter().enumerate() {
                    if kit::icon_chip(ui, self.faction == i, assets::syndicate_logo(tag), crate::i18n::tr(name)).clicked() && self.faction != i {
                        self.faction = i;
                        self.row = 0;
                    }
                }
            },
            |ui| {
                if expiry.is_some() {
                    kit::pill(ui, &trf("Rotates in {}", &[&format::until(expiry)]), TEXT_SECONDARY());
                }
                if let Some(f) = book.as_ref().and_then(|b| b.zariman_faction.as_deref()).filter(|_| tag == "ZarimanSyndicate") {
                    kit::pill(ui, &trf("vs {}", &[&f]), theme::faction_color(f));
                }
            },
        );
        ui.add_space(8.0);

        let drops = state.drops.clone();
        let db = state.db.clone();
        let rows = match (live, seeded) {
            (Some(b), _) => b
                .jobs
                .iter()
                .map(|j| Row {
                    title: book.as_ref().and_then(|bk| bk.job_names.get(&j.job_type)).map_or(Cow::from(j.name.as_str()), |n| n.as_str().into()),
                    task: None,
                    min_level: j.min_level,
                    max_level: j.max_level,
                    mastery_req: j.mastery_req,
                    rotation: j.rotation,
                    ally: None,
                    table: table_for(drops.as_deref(), tag, j.min_level, j.max_level, j.vault),
                })
                .collect(),
            (None, Some(list)) => seeded_rows(list, tag, drops.as_deref(), db.as_deref()),
            (None, None) => Vec::new(),
        };
        if rows.is_empty() {
            let loading = (live.is_none() && state.bounties_requested) || drops.is_none();
            if loading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(tr("Loading bounties…")).color(TEXT_MUTED()));
                });
            } else {
                kit::empty(ui, tr("No bounties"), tr("None are listed for this faction right now."));
            }
            return;
        }
        self.row = self.row.min(rows.len() - 1);

        let list = |ui: &mut Ui, row: &mut usize| {
            for (i, r) in rows.iter().enumerate() {
                if bounty_row(ui, r, i == *row).clicked() {
                    *row = i;
                }
            }
        };
        if ui.available_width() >= SPLIT_WIDTH {
            let width = (ui.available_width() * 0.45).clamp(380.0, 600.0);
            egui::Panel::left("bounty-list")
                .resizable(false)
                .exact_size(width)
                .frame(egui::Frame::new().inner_margin(Margin { left: 0, right: 12, top: 0, bottom: 0 }))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().id_salt("bounty-list").auto_shrink(false).show(ui, |ui| list(ui, &mut self.row));
                });
            egui::ScrollArea::vertical().id_salt("bounty-drops").auto_shrink(false).show(ui, |ui| {
                drop_table(ui, &rows[self.row], drops.is_some());
            });
        } else {
            egui::ScrollArea::vertical().id_salt("bounty-stack").auto_shrink(false).show(ui, |ui| {
                list(ui, &mut self.row);
                ui.add_space(8.0);
                drop_table(ui, &rows[self.row], drops.is_some());
            });
        }
    }
}

fn table_for<'a>(drops: Option<&'a DropIndex>, tag: &str, min: u32, max: u32, vault: bool) -> Option<&'a BountyTable> {
    drops?.bounty_for(bounty_source(tag)?, min, max, vault)
}

/// Seeded bounties pay from the faction's level brackets in board order: the
/// n-th bounty from the n-th bracket.
fn seeded_rows<'a>(
    list: &'a [fluffyframe_core::services::bounty_cycle::SeededBounty],
    tag: &str,
    drops: Option<&'a DropIndex>,
    db: Option<&'a ItemDb>,
) -> Vec<Row<'a>> {
    let tables: Vec<&BountyTable> = drops.zip(bounty_source(tag)).map(|(d, src)| d.bounty_tables(src).collect()).unwrap_or_default();
    list.iter()
        .enumerate()
        .map(|(i, b)| {
            let region = db.and_then(|db| db.regions.get(&b.node));
            let title = match region {
                // Some node names say the mission already ("Exterminate: Scaldra").
                Some(r) if r.name.contains(':') || r.mission_type.is_empty() => Cow::from(r.name.as_str()),
                Some(r) => format!("{} · {}", crate::i18n::tr_any(&r.mission_type), r.name).into(),
                None => Cow::from(b.node.as_str()),
            };
            let table = tables.get(i).copied();
            Row {
                title,
                task: Some((b.title.as_str(), b.task.as_str())),
                min_level: table.map_or(region.map_or(0, |r| r.min_level), |t| t.min_level),
                max_level: table.map_or(region.map_or(0, |r| r.max_level), |t| t.max_level),
                mastery_req: 0,
                rotation: None,
                ally: b.ally.as_deref(),
                table,
            }
        })
        .collect()
}

/// A selectable bounty: levels and title, pills on the right, then its task.
fn bounty_row(ui: &mut Ui, r: &Row, selected: bool) -> egui::Response {
    let fill = if selected { alpha(ACCENT(), 0.10) } else { BG_WELL() };
    let stroke = if selected { alpha(ACCENT(), 0.6) } else { egui::Color32::TRANSPARENT };
    let frame = egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, stroke))
        .corner_radius(RADIUS_SM())
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            kit::split_row(
                ui,
                |ui| {
                    let levels = trf("Lv {}–{}", &[&r.min_level, &r.max_level]);
                    ui.add_sized(vec2(64.0, 16.0), egui::Label::new(RichText::new(levels).size(12.0).color(TEXT_MUTED())));
                    ui.label(RichText::new(r.title.as_ref()).family(theme::body_strong()));
                },
                |ui| {
                    if let Some(rot) = r.rotation {
                        kit::pill(ui, &trf("Rot {}", &[&rot]), INFO());
                    }
                    if r.mastery_req > 0 {
                        kit::pill(ui, &trf("MR {}", &[&r.mastery_req]), TEXT_SECONDARY());
                    }
                    if let Some(ally) = r.ally {
                        kit::pill(ui, ally, POSITIVE());
                    }
                },
            );
            if let Some((name, task)) = r.task {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(64.0 + ui.spacing().item_spacing.x);
                    ui.label(RichText::new(name).size(12.0).color(ACCENT()).family(theme::body_strong()));
                    ui.label(RichText::new(task).size(12.0).color(TEXT_SECONDARY()));
                });
            }
        });
    ui.add_space(3.0);
    ui.interact(frame.response.rect, ui.scope_id().with(("bounty", r.title.as_ref(), r.min_level, r.max_level)), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The selected bounty's rewards by stage: the job's rotation, or every rotation
/// of a fixed table.
fn drop_table(ui: &mut Ui, r: &Row, loaded: bool) {
    kit::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(r.title.as_ref()).font(FontId::new(16.0, theme::display_bold())));
        let Some(table) = r.table else {
            let text = if loaded { tr("No drop table for this bounty in the drop data.") } else { tr("Loading drop tables…") };
            ui.label(RichText::new(text).color(TEXT_MUTED()));
            return;
        };
        let mut rotations: Vec<&(char, Vec<BountyReward>)> = table.rotations.iter().filter(|(c, _)| r.rotation.is_none_or(|rot| *c == rot)).collect();
        // A job whose rotation the drop data lacks still shows the table it has.
        if rotations.is_empty() {
            rotations = table.rotations.iter().collect();
        }
        let rot = r.rotation.map(|rot| trf(" · Rotation {}", &[&rot])).unwrap_or_default();
        kit::caption(ui, &trf("Lv {}–{}{}", &[&table.min_level, &table.max_level, &rot]));
        for (_, rewards) in rotations {
            let mut stage = None;
            for w in rewards {
                if stage != Some(w.stage.as_str()) {
                    stage = Some(w.stage.as_str());
                    ui.add_space(6.0);
                    ui.label(RichText::new(&w.stage).size(11.5).color(TEXT_MUTED()).family(theme::body_strong()));
                }
                kit::split_row(
                    ui,
                    |ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(6.0, 6.0), egui::Sense::hover());
                        ui.painter().circle_filled(rect.center(), 3.0, rarity_color(&w.rarity));
                        ui.label(RichText::new(&w.item).size(12.5));
                    },
                    |ui| {
                        ui.label(RichText::new(format!("{:.2}%", w.chance)).size(12.0).color(TEXT_SECONDARY()));
                    },
                );
            }
        }
    });
}

fn rarity_color(rarity: &str) -> egui::Color32 {
    match rarity {
        "Common" => TEXT_MUTED(),
        "Uncommon" => INFO(),
        "Rare" => ACCENT(),
        _ => POSITIVE(),
    }
}
