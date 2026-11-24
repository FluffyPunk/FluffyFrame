//! Navigation sidebar (original: `Sidebar.svelte`, `src/lib/viewRegistry.ts`).

use egui::{FontId, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::assets;
use crate::components::ui;
use crate::theme::{self, *};
use crate::views::ViewName;

pub const WIDTH: f32 = 250.0;
pub const COLLAPSED_WIDTH: f32 = 64.0;

fn nav_item(ui: &mut Ui, selected: bool, collapsed: bool, icon: egui::ImageSource<'static>, label: &str) -> egui::Response {
    let height = 40.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, RADIUS_MD(), alpha(ACCENT(), 0.13));
        painter.rect_filled(egui::Rect::from_min_size(rect.min, vec2(3.0, height)), 1.0, ACCENT());
    } else if resp.hovered() {
        painter.rect_filled(rect, RADIUS_MD(), theme::alpha(TEXT(), 0.05));
    }
    let icon_rect = egui::Rect::from_center_size(pos2(rect.left() + 26.0, rect.center().y), vec2(24.0, 24.0));
    let tint = if selected { ACCENT_BRIGHT() } else { TEXT() };
    egui::Image::new(icon).tint(tint).paint_at(ui, icon_rect);
    if !collapsed {
        let color = if selected { ACCENT() } else { TEXT() };
        ui.painter().text(
            pos2(rect.left() + 48.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            FontId::new(17.0, theme::display()),
            color,
        );
    }
    if collapsed { resp.on_hover_text(label) } else { resp }
}

pub fn show(ui: &mut Ui, current: &mut ViewName, hidden: &[String], collapsed: &mut bool) {
    ui.add_space(14.0);
    let label = if *collapsed { "›" } else { "‹   Collapse" };
    let resp = ui.add(egui::Label::new(RichText::new(label).font(FontId::new(16.0, theme::display())).color(TEXT_SECONDARY())).sense(Sense::click()));
    if resp.clicked() {
        *collapsed = !*collapsed;
    }
    ui.add_space(10.0);
    for view in ViewName::SIDEBAR {
        if view.hideable() && hidden.iter().any(|h| h == view.id()) {
            continue;
        }
        if nav_item(ui, *current == view, *collapsed, assets::view_icon(view), view.label()).clicked() {
            *current = view;
        }
        ui.add_space(3.0);
    }
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
        ui.add_space(12.0);
        let feedback = nav_item(ui, false, *collapsed, assets::TRADE, "Feedback");
        if feedback.clicked() {
            ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/WFHelper/WFHelper/issues"));
        }
    });
    let r = ui.max_rect();
    ui.painter().vline(r.right() + 7.0, r.y_range(), Stroke::new(1.0, BORDER_SUBTLE()));
    let _ = ui::caption;
}
