//! Title strip and status bar (original: `Titlebar.svelte`, `StatusBar.svelte`).

use egui::{Color32, CursorIcon, FontId, Rect, RichText, Sense, Stroke, StrokeKind, Ui, ViewportCommand, pos2, vec2};
use fluffyframe_core::config::app_meta;
use fluffyframe_core::config::settings::InventorySource;

use super::format;
use crate::assets;
use crate::ipc::{Backend, InventoryCmd};
use crate::stores::{self, AppState};
use crate::theme::{self, *};

/// Custom title bar (the window is frameless, like the original): drag to move,
/// double-click to maximize, logo and name centered, window buttons on the right.
pub fn title_strip(ui: &mut Ui) {
    let (rect, bar) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let ctx = ui.ctx().clone();
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

    // egui only centers single widgets, so lay out the logo + name by hand.
    let (icon, gap) = (18.0, 6.0);
    let name = ui.painter().layout_no_wrap(app_meta::APP_NAME.to_uppercase(), FontId::new(13.0, theme::display_bold()), TEXT());
    let left = rect.center().x - (icon + gap + name.size().x) / 2.0;
    let icon_rect = Rect::from_center_size(pos2(left + icon / 2.0, rect.center().y), vec2(icon, icon));
    egui::Image::new(assets::LOGO).paint_at(ui, icon_rect);
    ui.painter().galley(pos2(left + icon + gap, rect.center().y - name.size().y / 2.0), name, TEXT());

    let mut x = rect.right();
    for control in [WindowControl::Close, WindowControl::Maximize, WindowControl::Minimize] {
        let button = Rect::from_min_max(pos2(x - CONTROL_WIDTH, rect.top()), pos2(x, rect.bottom()));
        x -= CONTROL_WIDTH;
        let resp = ui.interact(button, ui.scope_id().with(("window-control", control as u8)), Sense::click());
        control.paint(ui, button, resp.hovered(), maximized);
        if resp.clicked() {
            ctx.send_viewport_cmd(match control {
                WindowControl::Close => ViewportCommand::Close,
                WindowControl::Maximize => ViewportCommand::Maximized(!maximized),
                WindowControl::Minimize => ViewportCommand::Minimized(true),
            });
        }
    }

    if bar.double_clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    } else if bar.drag_started_by(egui::PointerButton::Primary) {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
}

/// Window button width (original `--size-titlebar-control: 2.875rem`).
const CONTROL_WIDTH: f32 = 44.0;

#[derive(Clone, Copy)]
enum WindowControl {
    Minimize,
    Maximize,
    Close,
}

impl WindowControl {
    fn paint(self, ui: &Ui, rect: Rect, hovered: bool, maximized: bool) {
        let painter = ui.painter();
        let close = matches!(self, WindowControl::Close);
        if hovered {
            painter.rect_filled(rect, 0.0, if close { Color32::from_rgb(0xc4, 0x2b, 0x1c) } else { alpha(TEXT(), 0.08) });
        }
        let color = if hovered && close { Color32::WHITE } else { TEXT_SECONDARY() };
        let stroke = Stroke::new(1.0, color);
        let c = rect.center();
        let h = 5.0;
        match self {
            WindowControl::Minimize => {
                painter.hline(c.x - h..=c.x + h, c.y, stroke);
            }
            WindowControl::Maximize if maximized => {
                // Two overlapping squares: restore.
                painter.rect_stroke(Rect::from_center_size(c + vec2(-1.5, 1.5), vec2(8.0, 8.0)), 0.0, stroke, StrokeKind::Middle);
                painter.line_segment([c + vec2(-2.5, -4.5), c + vec2(5.5, -4.5)], stroke);
                painter.line_segment([c + vec2(5.5, -4.5), c + vec2(5.5, 3.5)], stroke);
            }
            WindowControl::Maximize => {
                painter.rect_stroke(Rect::from_center_size(c, vec2(10.0, 10.0)), 0.0, stroke, StrokeKind::Middle);
            }
            WindowControl::Close => {
                painter.line_segment([c + vec2(-h, -h), c + vec2(h, h)], stroke);
                painter.line_segment([c + vec2(-h, h), c + vec2(h, -h)], stroke);
            }
        }
    }
}

/// A frameless window has no OS resize border: a few pixels at each edge and
/// corner start a native resize instead. Call once per frame.
pub fn resize_edges(ctx: &egui::Context) {
    const EDGE: f32 = 5.0;
    let (pos, pressed, maximized, screen) = ctx.input(|i| {
        (i.pointer.hover_pos(), i.pointer.primary_pressed(), i.viewport().maximized.unwrap_or(false), i.content_rect())
    });
    let Some(pos) = pos else { return };
    if maximized {
        return;
    }
    let (l, r) = (pos.x - screen.left() < EDGE, screen.right() - pos.x < EDGE);
    let (t, b) = (pos.y - screen.top() < EDGE, screen.bottom() - pos.y < EDGE);
    use egui::viewport::ResizeDirection as D;
    let (dir, cursor) = match (l, r, t, b) {
        (true, _, true, _) => (D::NorthWest, CursorIcon::ResizeNorthWest),
        (_, true, true, _) => (D::NorthEast, CursorIcon::ResizeNorthEast),
        (true, _, _, true) => (D::SouthWest, CursorIcon::ResizeSouthWest),
        (_, true, _, true) => (D::SouthEast, CursorIcon::ResizeSouthEast),
        (true, ..) => (D::West, CursorIcon::ResizeWest),
        (_, true, ..) => (D::East, CursorIcon::ResizeEast),
        (_, _, true, _) => (D::North, CursorIcon::ResizeNorth),
        (.., true) => (D::South, CursorIcon::ResizeSouth),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if pressed {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

/// "HH:MM" today, "Mon HH:MM" otherwise.
fn clock(t: chrono::DateTime<chrono::Utc>) -> String {
    let local = t.with_timezone(&chrono::Local);
    if local.date_naive() == chrono::Local::now().date_naive() { local.format("%H:%M").to_string() } else { format::local_time(t) }
}

/// Colored dot and text.
/// warframe.market status dot: green online, blue in game, grey invisible.
pub fn wfm_status_color(status: Option<fluffyframe_core::services::wfm_presence::WfmStatus>) -> Color32 {
    use fluffyframe_core::services::wfm_presence::WfmStatus;
    match status {
        Some(WfmStatus::Online) => POSITIVE(),
        Some(WfmStatus::InGame) => INFO(),
        Some(WfmStatus::Invisible) | None => TEXT_MUTED(),
    }
}

fn status(ui: &mut Ui, dot: Color32, text: String) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new("●").size(9.0).color(dot));
        ui.label(RichText::new(text).size(11.5).color(TEXT_MUTED()));
    })
    .response
}

pub fn show(ui: &mut Ui, state: &AppState, backend: &Backend) {
    let settings = stores::settings(backend);
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let small = |t: String| RichText::new(t).size(11.5).color(TEXT_MUTED());
        let (dot, text) = match (&state.db, &state.loading) {
            (_, Some(l)) => (WARNING(), l.clone()),
            (Some(_), None) => {
                let t = state.world.as_ref().and_then(|w| w.fetched_at).map(clock).unwrap_or_default();
                (POSITIVE(), format!("WF data {t}").trim_end().to_string())
            }
            (None, None) => (NEGATIVE(), "WF data unavailable".into()),
        };
        status(ui, dot, text).on_hover_text("Game data and world state");
        match &state.inventory {
            Some(inv) => {
                let updated = inv.fetched_at.map(|t| format!(" · updated {}", clock(t))).unwrap_or_default();
                ui.label(small(format!(
                    "{} items loaded · {}{updated}",
                    format::int(state.rows.len() as u64),
                    inv.source.unwrap_or(settings.inventory_source).label()
                )));
            }
            None => {
                ui.label(small("No inventory loaded".into()));
            }
        }
        if settings.inventory_source == InventorySource::Game {
            if state.helper.running {
                ui.add(egui::Spinner::new().size(11.0));
                ui.label(small("Reading game…".into()));
            } else {
                if let Some(f) = state.helper.last_failure {
                    ui.label(RichText::new("⚠").color(WARNING())).on_hover_text(f.hint());
                }
                if ui.add(egui::Label::new(small("⟳ Refresh".into())).sense(egui::Sense::click())).on_hover_text("Read inventory from the game now").clicked() {
                    backend.inventory(InventoryCmd::RefreshNow);
                }
            }
        }
        let (dot, text) = match (&state.ee_log, state.logged_in) {
            (Some(_), true) => (POSITIVE(), "In game"),
            (Some(_), false) => (WARNING(), "EE.log found"),
            (None, _) => (NEGATIVE(), "EE.log not found"),
        };
        status(ui, dot, text.into()).on_hover_text(
            state.ee_log.as_ref().map(|p| p.display().to_string()).unwrap_or("Set the EE.log path in Settings".into()),
        );
        if state.price_queue > 0 {
            ui.add(egui::Spinner::new().size(11.0));
            ui.label(small(format!("Pricing {} items", state.price_queue)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(11.0).color(Color32::from_gray(90)));
            if let Some(s) = &state.session {
                // Right to left: the name, then its status dot before it.
                let name = ui.label(small(format!("WFM: {}", s.ingame_name.as_deref().unwrap_or("signed in"))));
                ui.add_space(-6.0);
                let dot = ui.label(RichText::new("●").size(9.0).color(wfm_status_color(state.wfm_status)));
                let tip = state.wfm_status.map_or("Status unknown", |s| s.label());
                name.on_hover_text(tip);
                dot.on_hover_text(tip);
            }
            let unread = state.history.len();
            ui.label(small(format!("🔔 {unread}"))).on_hover_ui(|ui| {
                for l in state.history.iter().rev().take(10) {
                    ui.label(format!("{}  {}", l.at.format("%H:%M"), l.text));
                }
            });
        });
    });
}
