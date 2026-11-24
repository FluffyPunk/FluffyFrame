//! Component kit matching the original's look (original: `ThemedPanel.svelte`,
//! `ThemedButton.svelte`, `SegmentedControl.svelte`, `HeaderTabs.svelte`,
//! `SummaryStrip.svelte`, `src/styles/components.css`).

use egui::{Align, Color32, CornerRadius, FontId, Frame, ImageSource, Layout, Margin, Response, RichText, Sense, Stroke, Ui, Vec2, pos2, vec2};

use crate::assets;
use crate::theme::{self, *};

/// Panel card: surface fill, faint gold border.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(BG_SURFACE())
        .stroke(Stroke::new(1.0, BORDER_STRONG()))
        .corner_radius(RADIUS_LG())
        .inner_margin(Margin::same(14))
        .show(ui, add)
        .inner
}

/// Inner card on a raised surface.
pub fn raised<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(BG_RAISED())
        .stroke(Stroke::new(1.0, BORDER()))
        .corner_radius(RADIUS_MD())
        .inner_margin(Margin::same(10))
        .show(ui, add)
        .inner
}

/// Large page title, with an optional count in parentheses.
pub fn page_title(ui: &mut Ui, title: &str, count: Option<usize>) {
    let text = match count {
        Some(n) => format!("{title} ({n})"),
        None => title.to_string(),
    };
    ui.label(RichText::new(text).font(FontId::new(32.0, theme::display_bold())).color(TEXT()));
}

pub fn page_subtitle(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).color(TEXT_SECONDARY()));
}

/// Underline tabs; returns true when the selection changed.
pub fn tabs<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, items: &[(T, String)]) -> bool {
    let mut changed = false;
    let start_y = ui.cursor().top();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (value, label) in items {
            let selected = *current == *value;
            let color = if selected { TEXT() } else { TEXT_SECONDARY() };
            let galley = ui.painter().layout_no_wrap(label.clone(), FontId::new(16.0, theme::display()), color);
            let size = vec2(galley.size().x + 28.0, 34.0);
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            let color = if resp.hovered() && !selected { TEXT() } else { color };
            ui.painter().galley_with_override_text_color(
                pos2(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0 - 2.0),
                galley,
                color,
            );
            if selected {
                let r = egui::Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.right_bottom());
                ui.painter().rect_filled(r, 0.0, TEXT());
            }
            if resp.clicked() && !selected {
                *current = *value;
                changed = true;
            }
        }
    });
    let w = ui.available_width();
    let y = ui.cursor().top() - ui.spacing().item_spacing.y + 1.0;
    let _ = start_y;
    ui.painter().hline(ui.cursor().left()..=ui.cursor().left() + w, y, Stroke::new(1.0, BORDER_SUBTLE()));
    ui.add_space(8.0);
    changed
}

/// Small uppercase badge.
pub fn pill(ui: &mut Ui, text: &str, color: Color32) -> Response {
    Frame::new()
        .fill(alpha(color, 0.14))
        .stroke(Stroke::new(1.0, alpha(color, 0.45)))
        .corner_radius(RADIUS_SM())
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| ui.label(RichText::new(text.to_uppercase()).size(10.5).color(color).family(theme::body_strong())))
        .response
}

/// Gold-outlined toggle chip (original: `.chip` buttons).
pub fn chip(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let (fill, stroke, color) = if selected {
        (alpha(ACCENT(), 0.14), ACCENT(), ACCENT_BRIGHT())
    } else {
        (BG_RAISED(), BORDER_STRONG(), TEXT_SECONDARY())
    };
    let btn = egui::Button::new(RichText::new(text).size(12.5).color(color))
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(RADIUS_SM())
        .min_size(vec2(0.0, 26.0));
    ui.add(btn)
}

/// Solid gold button.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).color(TEXT_ON_ACCENT()).family(theme::body_strong()))
            .fill(ACCENT())
            .stroke(Stroke::NONE)
            .corner_radius(RADIUS_MD())
            .min_size(vec2(0.0, 34.0)),
    )
}

/// Outlined secondary button.
pub fn ghost_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(12.5).color(TEXT_SECONDARY()))
            .fill(BG_RAISED())
            .stroke(Stroke::new(1.0, BORDER_STRONG()))
            .corner_radius(RADIUS_SM())
            .min_size(vec2(0.0, 28.0)),
    )
}

/// UPPERCASE muted caption.
pub fn caption(ui: &mut Ui, text: &str) -> Response {
    ui.label(RichText::new(text.to_uppercase()).size(11.0).color(TEXT_MUTED()).family(theme::body_strong()))
}

/// Card heading in the display font.
pub fn card_title(ui: &mut Ui, text: &str) -> Response {
    ui.label(RichText::new(text).font(FontId::new(17.0, theme::display_bold())).color(TEXT()))
}

pub fn icon(ui: &mut Ui, src: ImageSource<'static>, size: f32) -> Response {
    ui.add(egui::Image::new(src).fit_to_exact_size(Vec2::splat(size)))
}

/// Remote image (item art) with a fixed box.
pub fn remote_image(ui: &mut Ui, url: Option<&str>, size: Vec2) -> Response {
    match url {
        Some(u) => ui.add(egui::Image::new(u.to_string()).fit_to_exact_size(size).maintain_aspect_ratio(true)),
        None => ui.allocate_response(size, Sense::hover()),
    }
}

/// Paints a remote image centred in `rect`, scaled to fit without stretching
/// (mod cards are portrait, arcane bases landscape).
pub fn paint_image_fit(ui: &Ui, url: &str, rect: egui::Rect) {
    let image = egui::Image::new(url.to_string()).fit_to_exact_size(rect.size()).maintain_aspect_ratio(true);
    let size = image.load_and_calc_size(ui, rect.size()).unwrap_or(rect.size());
    image.paint_at(ui, egui::Rect::from_center_size(rect.center(), size));
}

/// Platinum icon + value ("—" when unknown).
pub fn plat(ui: &mut Ui, value: Option<f64>, size: f32) {
    ui.spacing_mut().item_spacing.x = 3.0;
    icon(ui, assets::PLATINUM, size);
    let text = match value {
        Some(v) if v.fract() == 0.0 || v >= 10.0 => format!("{v:.0}"),
        Some(v) => format!("{v:.1}"),
        None => "-".into(),
    };
    ui.label(RichText::new(text).family(theme::body_strong()).color(if value.is_some() { TEXT() } else { TEXT_MUTED() }));
}

pub fn ducats(ui: &mut Ui, value: Option<u32>, size: f32) {
    ui.spacing_mut().item_spacing.x = 3.0;
    icon(ui, assets::DUCATS_SMALL, size);
    let text = value.map(|v| v.to_string()).unwrap_or_else(|| "-".into());
    ui.label(RichText::new(text).family(theme::body_strong()).color(if value.is_some() { ACCENT() } else { TEXT_MUTED() }));
}

/// Thin progress bar.
pub fn progress(ui: &mut Ui, frac: f32, color: Color32, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let r = CornerRadius::same((height / 2.0) as u8);
    ui.painter().rect_filled(rect, r, BG_HOVER());
    let mut fill = rect;
    fill.set_width(rect.width() * frac.clamp(0.0, 1.0));
    if fill.width() > 0.5 {
        ui.painter().rect_filled(fill, r, color);
    }
    resp
}

/// Lays out `n` fixed-size cards in a virtualized responsive grid.
pub fn card_grid(ui: &mut Ui, min_width: f32, height: f32, n: usize, add: impl FnMut(&mut Ui, usize)) {
    grid_sections(ui, vec![GridSection::new(min_width, height, n, add)]);
}

type CellFn<'a> = Box<dyn FnMut(&mut Ui, usize) + 'a>;
type HeightFn<'a> = Box<dyn Fn(usize) -> f32 + 'a>;

/// One block of cells inside [`grid_sections`].
pub struct GridSection<'a> {
    title: Option<String>,
    min_width: f32,
    height: f32,
    /// Per-cell height; each row is as tall as its tallest cell, like a CSS grid.
    height_of: Option<HeightFn<'a>>,
    /// Height as `min(width, max_width) × ratio` (fixed-shape cards).
    aspect: Option<(f32, f32)>,
    gap: f32,
    n: usize,
    add: CellFn<'a>,
}

impl<'a> GridSection<'a> {
    pub fn new(min_width: f32, height: f32, n: usize, add: impl FnMut(&mut Ui, usize) + 'a) -> Self {
        Self { title: None, min_width, height, height_of: None, aspect: None, gap: 12.0, n, add: Box::new(add) }
    }

    /// Cells keep a shape: height = `min(cell width, max_width) × ratio`.
    pub fn aspect(mut self, ratio: f32, max_width: f32) -> Self {
        self.aspect = Some((ratio, max_width));
        self
    }

    /// Cells sized by content (rows take their tallest cell); `height` becomes unused.
    pub fn heights(mut self, height_of: impl Fn(usize) -> f32 + 'a) -> Self {
        self.height_of = Some(Box::new(height_of));
        self
    }

    /// Caption shown above the section (only when it has cells).
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }
}

/// Grids stacked in one scroll area; only visible cells are built.
pub fn grid_sections(ui: &mut Ui, mut sections: Vec<GridSection<'_>>) {
    const TITLE_H: f32 = 34.0;
    let avail = ui.available_width() - 14.0;
    egui::ScrollArea::vertical().auto_shrink(false).show_viewport(ui, |ui, viewport| {
        let origin = ui.max_rect().min;
        let mut y = 0.0;
        for (si, s) in sections.iter_mut().enumerate().filter(|(_, s)| s.n > 0) {
            if let Some(title) = &s.title {
                if y > 0.0 {
                    y += 12.0;
                }
                let rect = egui::Rect::from_min_size(origin + vec2(0.0, y), vec2(avail, TITLE_H));
                if ui.is_rect_visible(rect) {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)), |ui| {
                        ui.label(RichText::new(title.to_uppercase()).font(FontId::new(14.0, theme::display())).color(TEXT_MUTED()));
                    });
                }
                y += TITLE_H;
            }
            let cols = (((avail + s.gap) / (s.min_width + s.gap)).floor() as usize).max(1);
            let width = (avail - s.gap * (cols - 1) as f32) / cols as f32;
            let rows = s.n.div_ceil(cols);
            // Row tops relative to the section start; tops[rows] is its height.
            let mut tops = Vec::with_capacity(rows + 1);
            let mut top = 0.0;
            for row in 0..rows {
                tops.push(top);
                let h = match (&s.height_of, s.aspect) {
                    (Some(f), _) => (row * cols..((row + 1) * cols).min(s.n)).map(f).fold(0.0, f32::max),
                    (None, Some((ratio, max_w))) => width.min(max_w) * ratio,
                    (None, None) => s.height,
                };
                top += h + s.gap;
            }
            tops.push(top);
            let first = tops.partition_point(|t| y + t <= viewport.min.y).saturating_sub(1);
            let last = tops.partition_point(|t| y + t < viewport.max.y).min(rows);
            for row in first..last {
                let height = tops[row + 1] - tops[row] - s.gap;
                for c in 0..cols {
                    let i = row * cols + c;
                    if i >= s.n {
                        break;
                    }
                    let min = origin + vec2(c as f32 * (width + s.gap), y + tops[row]);
                    let rect = egui::Rect::from_min_size(min, vec2(width, height));
                    ui.scope_builder(egui::UiBuilder::new().id_salt((si, i)).max_rect(rect).layout(Layout::top_down(Align::Min)), |ui| {
                        ui.set_width(width);
                        ui.set_height(height);
                        (s.add)(ui, i);
                    });
                }
            }
            y += top;
        }
        ui.set_min_size(vec2(avail, y));
    });
}

/// Clickable card frame of a fixed size; returns the click response.
pub fn grid_card(ui: &mut Ui, highlight: Option<Color32>, add: impl FnOnce(&mut Ui)) -> Response {
    let rect = ui.max_rect();
    let resp = ui.interact(rect, ui.next_auto_id(), Sense::click());
    let stroke = match highlight {
        Some(c) => Stroke::new(1.0, c),
        None if resp.hovered() => Stroke::new(1.0, ACCENT_DIM()),
        None => Stroke::new(1.0, BORDER_STRONG()),
    };
    ui.painter().rect(rect, RADIUS_LG(), BG_SURFACE(), stroke, egui::StrokeKind::Inside);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(1.0)).layout(Layout::top_down(Align::Min)));
    add(&mut child);
    resp
}

/// Big number + label, as in Mastery's summary strip.
pub fn big_stat(ui: &mut Ui, value: &str, label: &str, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(value).font(FontId::new(40.0, theme::display_bold())).color(color));
        ui.label(RichText::new(label).font(FontId::new(22.0, theme::display())).color(TEXT_SECONDARY()));
    });
}

/// Circular progress ring with a centred percentage.
pub fn ring(ui: &mut Ui, frac: f32, size: f32, color: Color32, caption_text: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let r = size / 2.0 - 6.0;
    let painter = ui.painter();
    painter.circle_stroke(c, r, Stroke::new(6.0, BG_HOVER()));
    let steps = 64;
    let end = std::f32::consts::TAU * frac.clamp(0.0, 1.0);
    let pts: Vec<_> = (0..=steps)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + end * i as f32 / steps as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    painter.add(egui::Shape::line(pts, Stroke::new(6.0, color)));
    painter.text(c - vec2(0.0, 6.0), egui::Align2::CENTER_CENTER, format!("{:.1}%", frac * 100.0), FontId::new(22.0, theme::display_bold()), TEXT());
    painter.text(c + vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, caption_text.to_uppercase(), FontId::new(10.0, theme::body_strong()), TEXT_MUTED());
}

/// "have/need ✓" / "✗".
pub fn have_need(ui: &mut Ui, have: u64, need: u64) {
    let ok = have >= need;
    ui.label(RichText::new(format!("{}/{}", compact_num(have), compact_num(need))).color(TEXT()));
    ui.label(RichText::new(if ok { "✔" } else { "✖" }).color(if ok { POSITIVE() } else { NEGATIVE() }).size(12.0));
}

/// 1_500_000 → "1.5M", 916 → "916".
pub fn compact_num(n: u64) -> String {
    match n {
        0..=9_999 => n.to_string(),
        // 999,950+ would print "1000.0K".
        10_000..=999_949 => trim_zero(format!("{:.1}K", n as f64 / 1e3)),
        _ => trim_zero(format!("{:.1}M", n as f64 / 1e6)),
    }
}

fn trim_zero(s: String) -> String {
    s.replace(".0K", "K").replace(".0M", "M")
}

/// Search input with a magnifier hint.
pub fn search(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    ui.add(
        egui::TextEdit::singleline(text)
            .hint_text(format!("🔍  {hint}"))
            .desired_width(width)
            .margin(Margin::symmetric(10, 6)),
    )
}

/// Empty-state box with a dashed feel.
pub fn empty(ui: &mut Ui, title: &str, body: &str) {
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(title).color(TEXT()).family(theme::body_strong()));
        ui.label(RichText::new(body).size(12.5).color(TEXT_MUTED()));
    });
    ui.add_space(20.0);
}

/// A labelled select-like ComboBox in the filter bar.
pub fn labelled<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.label(RichText::new(label).size(11.5).color(TEXT_MUTED()));
    add(ui)
}


/// Segmented control (original: `SegmentedControl.svelte`); returns true on change.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    Frame::new()
        .fill(BG_RAISED())
        .stroke(Stroke::new(1.0, BORDER()))
        .corner_radius(RADIUS_SM())
        .inner_margin(Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (v, label) in options {
                    let selected = *value == *v;
                    let btn = egui::Button::new(RichText::new(*label).size(12.5).color(if selected { TEXT_ON_ACCENT() } else { TEXT_SECONDARY() }))
                        .fill(if selected { ACCENT() } else { egui::Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(RADIUS_SM())
                        .min_size(vec2(0.0, 24.0));
                    if ui.add(btn).clicked() && !selected {
                        *value = *v;
                        changed = true;
                    }
                }
            });
        });
    changed
}

/// Theme gallery: one swatch card per original preset; returns the picked id.
pub fn theme_picker(ui: &mut Ui, current: &str) -> Option<String> {
    let mut picked = None;
    let presets = theme::preset_list();
    let w = 172.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for (id, label, p) in presets {
            let selected = id == current;
            let (rect, resp) = ui.allocate_exact_size(vec2(w, 64.0), Sense::click());
            let stroke = if selected {
                Stroke::new(2.0, ACCENT())
            } else if resp.hovered() {
                Stroke::new(1.0, ACCENT_DIM())
            } else {
                Stroke::new(1.0, BORDER_STRONG())
            };
            let painter = ui.painter();
            painter.rect(rect, RADIUS_MD(), p.bg_base, stroke, egui::StrokeKind::Inside);
            let inner = rect.shrink(8.0);
            painter.rect_filled(egui::Rect::from_min_size(inner.min, vec2(inner.width(), 22.0)), RADIUS_SM(), p.bg_surface);
            painter.text(inner.min + vec2(6.0, 11.0), egui::Align2::LEFT_CENTER, label, FontId::new(13.0, theme::body_strong()), p.text_primary);
            for (i, c) in [p.accent, p.accent_bright, p.info, p.success, p.danger].iter().enumerate() {
                painter.circle_filled(pos2(inner.left() + 8.0 + i as f32 * 16.0, inner.bottom() - 8.0), 6.0, *c);
            }
            if resp.on_hover_text(label).clicked() {
                picked = Some(id.to_string());
            }
        }
    });
    picked
}

#[cfg(test)]
mod tests {
    #[test]
    fn compact() {
        assert_eq!(super::compact_num(916), "916");
        assert_eq!(super::compact_num(65_000), "65K");
        assert_eq!(super::compact_num(1_500_000), "1.5M");
    }
}
