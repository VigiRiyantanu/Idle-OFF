//! Visual theme, typography, spacing, and design palette for the IdleOff mod menu UI.
//!
//! # Mod Menu Aesthetics
//! - Palette directly sampled from `assets/background.png`:
//!   - `COLOR_SURFACE` (`#141316`): Obsidian black from character shadows.
//!   - `COLOR_TEXT` (`#F2EFEB`): Misty ethereal off-white.
//!   - `COLOR_MUTED` (`#8C8782`): Ash gray from thorn halo.
//!   - `COLOR_ACCENT` (`#D8D2C7`): Bone ivory from illuminated feathers.
//! - Sharp geometry: 2 px corner radius on all cards, buttons, switches, and tabs.
//! - Thin 1 px borders with high-contrast accent focal points.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, Style, TextStyle, Theme,
    Visuals,
};
use std::sync::Arc;

/// Embedded Inter-Regular font (SIL Open Font License).
pub const FONT_INTER: &[u8] = include_bytes!("../../assets/inter.otf");

// --- Palette Constants ---
/// Deep obsidian black sampled from the character cloak and hair (#141316).
pub const COLOR_SURFACE: Color32 = Color32::from_rgb(0x14, 0x13, 0x16);

/// Misty ethereal off-white sampled from upper atmospheric background (#F2EFEB).
pub const COLOR_TEXT: Color32 = Color32::from_rgb(0xF2, 0xEF, 0xEB);

/// Neutral ash stone gray sampled from the thorn halo and shadows (#8C8782).
pub const COLOR_MUTED: Color32 = Color32::from_rgb(0x8C, 0x87, 0x82);

/// Luminous bone ivory sampled from illuminated right-wing feathers (#D8D2C7).
pub const COLOR_ACCENT: Color32 = Color32::from_rgb(0xD8, 0xD2, 0xC7);

/// Semi-transparent dark overlay panel fill (alpha ~205) for high contrast and readability.
pub const OVERLAY_FILL: Color32 = Color32::from_rgba_premultiplied(0x12, 0x11, 0x14, 205);

/// Darker sidebar background fill (alpha ~225).
pub const SIDEBAR_FILL: Color32 = Color32::from_rgba_premultiplied(0x0E, 0x0D, 0x10, 225);

/// Header strip background fill (alpha ~235).
pub const HEADER_FILL: Color32 = Color32::from_rgba_premultiplied(0x0A, 0x09, 0x0C, 235);

/// Subtle card and panel border stroke color.
pub const CARD_BORDER: Color32 = Color32::from_rgba_premultiplied(0xD8, 0xD2, 0xC7, 40);

/// Accent border stroke for active borders and focus.
pub const BORDER_ACCENT: Color32 = Color32::from_rgba_premultiplied(0xD8, 0xD2, 0xC7, 180);

/// Hairline divider stroke color for separating setting rows.
pub const HAIRLINE_DIVIDER: Color32 = Color32::from_rgba_premultiplied(0x8C, 0x87, 0x82, 45);

// --- Geometry: Sharp Mod Menu Radius ---
pub const CORNER_RADIUS_SHARP: CornerRadius = CornerRadius::same(2);

// --- Spacing Scale (compact mod menu scale) ---
pub const SPACING_XS: f32 = 4.0;
pub const SPACING_SM: f32 = 8.0;
pub const SPACING_MD: f32 = 12.0;

// --- Typography Font Scale ---
pub const FONT_SIZE_HEADER_TITLE: f32 = 11.0;
pub const FONT_SIZE_SECTION: f32 = 10.0;
pub const FONT_SIZE_TAB: f32 = 11.0;
pub const FONT_SIZE_BODY: f32 = 12.0;
pub const FONT_SIZE_READOUT: f32 = 18.0;
pub const FONT_SIZE_READOUT_LABEL: f32 = 9.0;

/// Installs the clean sans-serif Inter font into egui font definitions.
pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(egui::FontData::from_static(FONT_INTER)),
    );

    // Set Inter as the first priority font for proportional text
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());

    // Also use Inter for numeric readouts with tabular/consistent numbers
    fonts
        .families
        .entry(FontFamily::Monospace)
        .or_default()
        .insert(0, "inter".to_owned());

    ctx.set_fonts(fonts);
}

/// Applies the dark cohesive visual style to egui.
pub fn configure_style(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.override_text_color = Some(COLOR_TEXT);
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.window_fill = Color32::TRANSPARENT;
    visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, COLOR_TEXT);

    let spacing = egui::style::Spacing {
        item_spacing: egui::vec2(SPACING_SM, SPACING_SM),
        window_margin: Margin::ZERO,
        ..Default::default()
    };

    let style = Style {
        text_styles: [
            (
                TextStyle::Heading,
                FontId::new(FONT_SIZE_BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(FONT_SIZE_BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(FONT_SIZE_BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(FONT_SIZE_READOUT, FontFamily::Monospace),
            ),
            (
                TextStyle::Small,
                FontId::new(FONT_SIZE_SECTION, FontFamily::Proportional),
            ),
        ]
        .into(),
        visuals,
        spacing,
        ..Default::default()
    };

    ctx.set_style_of(Theme::Dark, style);
}
