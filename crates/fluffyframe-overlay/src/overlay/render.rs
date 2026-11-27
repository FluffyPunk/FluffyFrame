//! Draws a [`Panel`] into a premultiplied RGBA pixmap: every platform shows
//! the same pixels.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

use super::Panel;

const PAD: f32 = 14.0;
const TITLE_PX: f32 = 20.0;
const LINE_PX: f32 = 16.0;
const GAP: f32 = 4.0;
const MAX_WIDTH: f32 = 640.0;
const RADIUS: f32 = 8.0;

const BACKGROUND: [u8; 4] = [12, 14, 20, 222];
const BORDER: [u8; 4] = [212, 175, 106, 160];
const TITLE: [u8; 4] = [232, 196, 124, 255];
const TEXT: [u8; 4] = [226, 230, 238, 255];

/// Fonts tried in order for each character (e.g. Latin, then Cyrillic).
pub struct Fonts(Vec<FontRef<'static>>);

impl Fonts {
    pub fn new(data: &[&'static [u8]]) -> Self {
        Self(data.iter().filter_map(|d| FontRef::try_from_slice(d).ok()).collect())
    }

    fn for_char(&self, c: char) -> Option<&FontRef<'static>> {
        self.0.iter().find(|f| f.glyph_id(c).0 != 0).or(self.0.first())
    }

    fn width(&self, text: &str, px: f32) -> f32 {
        text.chars().filter_map(|c| self.for_char(c).map(|f| f.as_scaled(PxScale::from(px)).h_advance(f.glyph_id(c)))).sum()
    }

    /// Draws `text` from `x` with its baseline at `y`, clipped at `max_x`.
    fn draw(&self, pixmap: &mut Pixmap, text: &str, (x, y): (f32, f32), px: f32, max_x: f32, color: [u8; 4]) {
        let mut pen = x;
        for c in text.chars() {
            let Some(font) = self.for_char(c) else { continue };
            let scaled = font.as_scaled(PxScale::from(px));
            let id = font.glyph_id(c);
            let advance = scaled.h_advance(id);
            if pen + advance > max_x {
                break;
            }
            if let Some(outline) = font.outline_glyph(id.with_scale_and_position(px, ab_glyph::point(pen, y))) {
                let bounds = outline.px_bounds();
                outline.draw(|gx, gy, coverage| {
                    blend(pixmap, bounds.min.x as i32 + gx as i32, bounds.min.y as i32 + gy as i32, color, coverage);
                });
            }
            pen += advance;
        }
    }
}

/// Source-over of a straight-alpha colour at `coverage` onto premultiplied pixels.
fn blend(pixmap: &mut Pixmap, x: i32, y: i32, color: [u8; 4], coverage: f32) {
    let (w, h) = (pixmap.width() as i32, pixmap.height() as i32);
    if x < 0 || y < 0 || x >= w || y >= h {
        return;
    }
    let a = f32::from(color[3]) / 255.0 * coverage.clamp(0.0, 1.0);
    let i = ((y * w + x) * 4) as usize;
    let px = &mut pixmap.data_mut()[i..i + 4];
    for c in 0..3 {
        px[c] = (f32::from(color[c]) * a + f32::from(px[c]) * (1.0 - a)).round() as u8;
    }
    px[3] = (a * 255.0 + f32::from(px[3]) * (1.0 - a)).round() as u8;
}

fn rgba(c: [u8; 4]) -> Color {
    Color::from_rgba8(c[0], c[1], c[2], c[3])
}

fn rounded_rect(w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(r, 0.0);
    pb.line_to(w - r, 0.0);
    pb.quad_to(w, 0.0, w, r);
    pb.line_to(w, h - r);
    pb.quad_to(w, h, w - r, h);
    pb.line_to(r, h);
    pb.quad_to(0.0, h, 0.0, h - r);
    pb.line_to(0.0, r);
    pb.quad_to(0.0, 0.0, r, 0.0);
    pb.close();
    pb.finish()
}

/// The panel's pixels (premultiplied RGBA, row-major, no padding).
pub fn render(panel: &Panel, fonts: &Fonts) -> Option<Pixmap> {
    let text_w = std::iter::once(fonts.width(&panel.title, TITLE_PX))
        .chain(panel.lines.iter().map(|l| fonts.width(l, LINE_PX)))
        .fold(0.0, f32::max);
    let width = (text_w + PAD * 2.0).clamp(160.0, MAX_WIDTH).ceil();
    let height = (PAD * 2.0 + TITLE_PX + panel.lines.len() as f32 * (LINE_PX + GAP) + if panel.lines.is_empty() { 0.0 } else { GAP * 2.0 }).ceil();
    let mut pixmap = Pixmap::new(width as u32, height as u32)?;

    let shape = rounded_rect(width - 1.0, height - 1.0, RADIUS)?;
    let mut paint = Paint { anti_alias: true, ..Default::default() };
    let at = Transform::from_translate(0.5, 0.5);
    paint.set_color(rgba(BACKGROUND));
    pixmap.fill_path(&shape, &paint, FillRule::Winding, at, None);
    paint.set_color(rgba(BORDER));
    pixmap.stroke_path(&shape, &paint, &Stroke { width: 1.0, ..Default::default() }, at, None);

    let max_x = width - PAD;
    let mut y = PAD + TITLE_PX * 0.8;
    fonts.draw(&mut pixmap, &panel.title, (PAD, y), TITLE_PX, max_x, TITLE);
    y += TITLE_PX * 0.2 + GAP * 2.0;
    for line in &panel.lines {
        y += LINE_PX;
        fonts.draw(&mut pixmap, line, (PAD, y), LINE_PX, max_x, TEXT);
        y += GAP;
    }
    Some(pixmap)
}

/// Premultiplied RGBA to premultiplied B, G, R, A byte order (Wayland
/// ARGB8888, X11 32-bit visuals, Windows DIBs).
pub fn to_bgra(pixmap: &Pixmap) -> Vec<u8> {
    let mut out = pixmap.data().to_vec();
    out.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p.swap(0, 2));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_a_panel_without_fonts() {
        let panel = Panel { title: "Scanning".into(), lines: vec!["one".into(), "two".into()] };
        let pixmap = render(&panel, &Fonts::new(&[])).unwrap();
        assert!(pixmap.width() >= 160);
        // Inside the panel is the background colour, outside the rounded corner is clear.
        let mid = pixmap.pixel(pixmap.width() / 2, pixmap.height() / 2).unwrap();
        assert_eq!(mid.alpha(), BACKGROUND[3]);
        assert_eq!(pixmap.pixel(0, 0).unwrap().alpha(), 0);
    }
}
