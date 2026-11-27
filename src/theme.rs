//! Design system (original: `src/styles/tokens.css`, `src/lib/theme/*`,
//! `src/config/themePresets.ts`).
//!
//! Colors come from the active [`Palette`] (one of the original's 16 presets) and
//! are read through the accessor functions below, so switching preset restyles
//! every view on the next frame. Fonts are the bundled Rajdhani and Barlow (OFL).
#![allow(non_snake_case)]

use std::sync::{Arc, RwLock};

use egui::{Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals};

use crate::theme_presets;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg_deep: Color32,
    pub bg_base: Color32,
    pub bg_surface: Color32,
    pub bg_raised: Color32,
    pub bg_hover: Color32,
    pub accent: Color32,
    pub accent_dim: Color32,
    pub accent_bright: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub info: Color32,
    pub border: Color32,
    pub border_strong: Color32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corners {
    Sharp,
    #[default]
    Soft,
    Round,
}

struct Active {
    palette: Palette,
    light: bool,
    corners: Corners,
}

static ACTIVE: RwLock<Option<Active>> = RwLock::new(None);

fn with<R>(f: impl FnOnce(&Active) -> R) -> R {
    let guard = ACTIVE.read().unwrap();
    match guard.as_ref() {
        Some(a) => f(a),
        None => {
            let (_, _, palette, light) = theme_presets::presets()[0];
            f(&Active { palette, light, corners: Corners::Soft })
        }
    }
}

fn p() -> Palette {
    with(|a| a.palette)
}

pub fn is_light() -> bool {
    with(|a| a.light)
}

// Surfaces
pub fn BG_DEEP() -> Color32 {
    p().bg_deep
}
pub fn BG_BASE() -> Color32 {
    p().bg_base
}
pub fn BG_SURFACE() -> Color32 {
    p().bg_surface
}
pub fn BG_RAISED() -> Color32 {
    p().bg_raised
}
pub fn BG_HOVER() -> Color32 {
    p().bg_hover
}
/// Card image well: between deep and surface.
pub fn BG_WELL() -> Color32 {
    mix(p().bg_deep, p().bg_surface, 0.5)
}
/// Sidebar: between deep and base.
pub fn BG_SIDEBAR() -> Color32 {
    mix(p().bg_deep, p().bg_surface, 0.35)
}

// Accent
pub fn ACCENT() -> Color32 {
    p().accent
}
pub fn ACCENT_DIM() -> Color32 {
    p().accent_dim
}
pub fn ACCENT_BRIGHT() -> Color32 {
    p().accent_bright
}

// Text
pub fn TEXT() -> Color32 {
    p().text_primary
}
pub fn TEXT_SECONDARY() -> Color32 {
    p().text_secondary
}
pub fn TEXT_MUTED() -> Color32 {
    p().text_muted
}
pub fn TEXT_ON_ACCENT() -> Color32 {
    if luminance(p().accent) > 0.55 { Color32::from_rgb(0x12, 0x15, 0x1a) } else { Color32::WHITE }
}

// Status
pub fn POSITIVE() -> Color32 {
    p().success
}
pub fn WARNING() -> Color32 {
    p().warning
}
pub fn NEGATIVE() -> Color32 {
    p().danger
}
pub fn INFO() -> Color32 {
    p().info
}

// Borders
pub fn BORDER() -> Color32 {
    p().border
}
pub fn BORDER_STRONG() -> Color32 {
    p().border_strong
}
pub fn BORDER_SUBTLE() -> Color32 {
    if is_light() { alpha(Color32::BLACK, 0.12) } else { alpha(Color32::WHITE, 0.12) }
}

// Relic eras (fixed, as in the original tokens).
pub const RELIC_LITH: Color32 = Color32::from_rgb(0x9c, 0xa3, 0xaf);
pub const RELIC_MESO: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);
pub const RELIC_NEO: Color32 = Color32::from_rgb(0x60, 0xa5, 0xfa);
pub const RELIC_AXI: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
pub const RELIC_REQUIEM: Color32 = Color32::from_rgb(0xa8, 0x55, 0xf7);

// Rarity frames for reward tiles.
pub const RARITY_COMMON: Color32 = Color32::from_rgb(0x8a, 0x5a, 0x3c);
pub const RARITY_UNCOMMON: Color32 = Color32::from_rgb(0xa8, 0xb0, 0xbc);
pub const RARITY_RARE: Color32 = Color32::from_rgb(0xd4, 0xa8, 0x43);

fn radius(base: u8) -> u8 {
    match with(|a| a.corners) {
        Corners::Sharp => base / 3,
        Corners::Soft => base,
        Corners::Round => base + base / 2 + 2,
    }
}
pub fn RADIUS_SM() -> u8 {
    radius(4)
}
pub fn RADIUS_MD() -> u8 {
    radius(7)
}
pub fn RADIUS_LG() -> u8 {
    radius(10)
}

/// Rajdhani 400: underline tabs.
pub fn display_light() -> FontFamily {
    FontFamily::Name("display-light".into())
}
/// Rajdhani 500: sidebar, chips, labels.
pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}
/// Rajdhani 600: page titles, card titles, buttons.
pub fn display_bold() -> FontFamily {
    FontFamily::Name("display-bold".into())
}
pub fn body_strong() -> FontFamily {
    FontFamily::Name("body-strong".into())
}

pub fn era_color(tier: &str) -> Color32 {
    match tier {
        "Lith" => RELIC_LITH,
        "Meso" => RELIC_MESO,
        "Neo" => RELIC_NEO,
        "Axi" => RELIC_AXI,
        "Requiem" | "Omnia" => RELIC_REQUIEM,
        _ => TEXT_MUTED(),
    }
}

pub fn faction_color(faction: &str) -> Color32 {
    match faction {
        "Grineer" => Color32::from_rgb(0xef, 0x44, 0x44),
        "Corpus" => Color32::from_rgb(0x60, 0xa5, 0xfa),
        "Infested" => Color32::from_rgb(0x4a, 0xde, 0x80),
        "Corrupted" | "Orokin" => Color32::from_rgb(0xf5, 0xc5, 0x42),
        "Sentient" => Color32::from_rgb(0xc0, 0x84, 0xfc),
        _ => TEXT_MUTED(),
    }
}

/// `color` at `alpha` (0..1) over transparent.
pub fn alpha(color: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), (a * color.a() as f32) as u8)
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn luminance(c: Color32) -> f32 {
    (0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32) / 255.0
}

const BARLOW_600: &[u8] = include_bytes!("../assets/fonts/barlow-latin-600-normal.ttf");
const GOLOS_600: &[u8] = include_bytes!("../assets/fonts/golos-cyrillic-600.ttf");

/// The overlay's fonts, tried in order per character (Latin, then Cyrillic).
pub fn overlay_fonts() -> Vec<&'static [u8]> {
    vec![BARLOW_600, GOLOS_600]
}

fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let add = |fonts: &mut FontDefinitions, name: &str, bytes: &'static [u8]| {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    };
    // The original's weights. The Rajdhani files carry its `letter-spacing: .03em`
    // in their advance widths, since egui has no global letter spacing.
    add(&mut fonts, "barlow-400", include_bytes!("../assets/fonts/barlow-latin-400-normal.ttf"));
    add(&mut fonts, "barlow-600", BARLOW_600);
    add(&mut fonts, "rajdhani-400", include_bytes!("../assets/fonts/rajdhani-latin-400-normal.ttf"));
    add(&mut fonts, "rajdhani-500", include_bytes!("../assets/fonts/rajdhani-latin-500-normal.ttf"));
    add(&mut fonts, "rajdhani-600", include_bytes!("../assets/fonts/rajdhani-latin-600-normal.ttf"));
    // Cyrillic, which Barlow and Rajdhani lack (Ukrainian UI): Golos Text behind
    // Barlow, Exo 2 behind Rajdhani, cut down to the Cyrillic block.
    add(&mut fonts, "golos-400", include_bytes!("../assets/fonts/golos-cyrillic-400.ttf"));
    add(&mut fonts, "golos-600", GOLOS_600);
    add(&mut fonts, "exo2-400", include_bytes!("../assets/fonts/exo2-cyrillic-400.ttf"));
    add(&mut fonts, "exo2-500", include_bytes!("../assets/fonts/exo2-cyrillic-500.ttf"));
    add(&mut fonts, "exo2-600", include_bytes!("../assets/fonts/exo2-cyrillic-600.ttf"));

    // Keep egui's defaults behind ours for symbols and emoji.
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let with_fallbacks = |first: &str, cyrillic: &str| {
        let mut v = vec![first.to_string(), cyrillic.to_string()];
        v.extend(fallbacks.iter().cloned());
        v
    };
    fonts.families.insert(FontFamily::Proportional, with_fallbacks("barlow-400", "golos-400"));
    fonts.families.insert(body_strong(), with_fallbacks("barlow-600", "golos-600"));
    fonts.families.insert(display_light(), with_fallbacks("rajdhani-400", "exo2-400"));
    fonts.families.insert(display(), with_fallbacks("rajdhani-500", "exo2-500"));
    fonts.families.insert(display_bold(), with_fallbacks("rajdhani-600", "exo2-600"));
    fonts
}

fn visuals(light: bool) -> Visuals {
    let mut v = if light { Visuals::light() } else { Visuals::dark() };
    let r = CornerRadius::same(RADIUS_MD());
    v.panel_fill = BG_BASE();
    v.window_fill = BG_SURFACE();
    v.window_stroke = Stroke::new(1.0, BORDER_STRONG());
    v.window_corner_radius = CornerRadius::same(RADIUS_LG());
    v.extreme_bg_color = BG_RAISED();
    v.faint_bg_color = mix(BG_SURFACE(), BG_RAISED(), 0.5);
    v.code_bg_color = BG_RAISED();
    v.override_text_color = Some(TEXT());
    v.hyperlink_color = ACCENT();
    v.selection.bg_fill = alpha(ACCENT(), 0.28);
    v.selection.stroke = Stroke::new(1.0, ACCENT());
    v.warn_fg_color = WARNING();
    v.error_fg_color = NEGATIVE();

    let w = &mut v.widgets;
    for (state, fill, stroke) in [
        (&mut w.noninteractive, BG_SURFACE(), BORDER()),
        (&mut w.inactive, BG_RAISED(), BORDER()),
        (&mut w.hovered, BG_HOVER(), BORDER_STRONG()),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.0, TEXT());
        state.corner_radius = r;
        state.expansion = 0.0;
    }
    w.active.bg_fill = alpha(ACCENT(), 0.25);
    w.active.weak_bg_fill = alpha(ACCENT(), 0.25);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT());
    w.active.fg_stroke = Stroke::new(1.0, ACCENT_BRIGHT());
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open = w.active;
    v.slider_trailing_fill = true;
    v
}

/// One-time setup: fonts and text styles.
pub fn install(ctx: &Context) {
    ctx.set_fonts(fonts());
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(25.0, display_bold())),
            (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(14.5, display_bold())),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.interact_size.y = 26.0;
        style.spacing.window_margin = Margin::same(16);
        style.spacing.menu_margin = Margin::same(8);
        style.interaction.selectable_labels = false;
    });
}

/// Activates a preset (unknown ids fall back to Orokin Gold), corner style and zoom.
pub fn apply(ctx: &Context, preset: &str, corners: Corners, scale: f32) {
    let all = theme_presets::presets();
    let (_, _, palette, light) = all.iter().find(|(id, ..)| *id == preset).copied().unwrap_or(all[0]);
    *ACTIVE.write().unwrap() = Some(Active { palette, light, corners });
    let v = visuals(light);
    ctx.set_visuals_of(egui::Theme::Dark, v.clone());
    ctx.set_visuals_of(egui::Theme::Light, v);
    ctx.set_theme(if light { egui::ThemePreference::Light } else { egui::ThemePreference::Dark });
    ctx.set_zoom_factor(scale.clamp(0.6, 2.5));
}

/// (id, label) of every preset, for pickers.
pub fn preset_list() -> Vec<(&'static str, &'static str, Palette)> {
    theme_presets::presets().into_iter().map(|(id, label, p, _)| (id, label, p)).collect()
}
