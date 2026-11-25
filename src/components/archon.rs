//! Archon shard pips (original: `ArchonShardPips.svelte`).

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, vec2};
use fluffyframe_core::services::inventory::{ArchonShard, ShardColor};

use crate::components::ui as kit;
use crate::stores::AppState;
use crate::theme::*;

/// Shard hues: fixed game colours, outside the theme presets.
fn shard_hex(color: ShardColor) -> Color32 {
    match color {
        ShardColor::Crimson => Color32::from_rgb(0xe2, 0x46, 0x5b),
        ShardColor::Amber => Color32::from_rgb(0xe8, 0xa6, 0x3a),
        ShardColor::Azure => Color32::from_rgb(0x3f, 0x9e, 0xe0),
        ShardColor::Emerald => Color32::from_rgb(0x3f, 0xc4, 0x89),
        ShardColor::Topaz => Color32::from_rgb(0xef, 0x8a, 0x3c),
        ShardColor::Violet => Color32::from_rgb(0xa2, 0x71, 0xe6),
    }
}

/// "Crimson · Tauforged", "Amber", "Unknown".
pub fn shard_name(shard: &ArchonShard) -> String {
    let hue = shard.color.map(ShardColor::label).unwrap_or("Unknown");
    if shard.tauforged { format!("{hue} · Tauforged") } else { hue.to_string() }
}

/// One socket: the shard's icon, a coloured dot when the hue or icon is
/// unknown, or a dim ring for an empty socket (`None`).
pub fn paint_socket(ui: &Ui, state: &AppState, shard: Option<&ArchonShard>, rect: Rect) {
    let Some(shard) = shard else {
        ui.painter().circle(rect.center(), rect.width() / 2.0 - 1.0, BG_WELL(), Stroke::new(1.0, BORDER()));
        return;
    };
    let icon = shard.color.zip(state.db.as_ref()).and_then(|(c, db)| db.image_url(&c.item_unique(shard.tauforged)));
    match icon {
        Some(url) => kit::paint_image_fit(ui, &url, rect),
        None => {
            let color = shard.color.map(shard_hex).unwrap_or(TEXT_MUTED());
            ui.painter().circle(rect.center(), rect.width() / 2.0 - 1.0, alpha(color, 0.72), Stroke::new(1.0, alpha(color, 0.55)));
        }
    }
}

/// Socketed shards in a row of 16px pips from `bottom_left`, with a tooltip
/// listing them (original size "lg").
pub fn shard_pips(ui: &Ui, state: &AppState, shards: &[ArchonShard], bottom_left: Pos2) {
    const SIZE: f32 = 16.0;
    const GAP: f32 = 5.0;
    if shards.is_empty() {
        return;
    }
    for (i, shard) in shards.iter().enumerate() {
        let rect = Rect::from_min_size(bottom_left + vec2(i as f32 * (SIZE + GAP), -SIZE), vec2(SIZE, SIZE));
        paint_socket(ui, state, Some(shard), rect);
    }
    let tip = shards.iter().map(shard_name).collect::<Vec<_>>().join(", ");
    let area = Rect::from_min_size(bottom_left + vec2(0.0, -SIZE), vec2(shards.len() as f32 * (SIZE + GAP), SIZE));
    ui.interact(area, ui.scope_id().with("shards"), Sense::hover())
        .on_hover_text(format!("{} archon shards: {tip}", shards.len()));
}
