//! Custom mod-menu interactive widgets for IdleOff.
//!
//! Includes:
//! - Sharp segmented 5-state timeout selector (5m, 15m, 30m, 1h, 2h) with accent highlight.
//! - Flat compact mod-menu toggle switches with row hover highlight.
//! - Live telemetry cards ("IDLE" and "SHUTDOWN IN") with dimmed paused state.
//! - Flat secondary action buttons with thin 1 px borders.
//! - Hairline dividers and uppercase section headers.

use crate::app::IdleTimeout;
use crate::ui::theme::{
    CARD_BORDER, COLOR_ACCENT, COLOR_MUTED, COLOR_SURFACE, COLOR_TEXT, CORNER_RADIUS_SHARP,
    FONT_SIZE_BODY, FONT_SIZE_READOUT, FONT_SIZE_READOUT_LABEL, FONT_SIZE_SECTION,
    HAIRLINE_DIVIDER, SPACING_XS,
};
use crate::util::format_countdown;
use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Vec2};

/// Uppercase section header label with letter-spaced aesthetic.
pub fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .font(FontId::proportional(FONT_SIZE_SECTION))
            .color(COLOR_MUTED)
            .strong(),
    );
}

/// Segmented control widget for choosing one of the 5 allowed idle timeouts.
///
/// Divides available width evenly across 5 segments with sharp 2 px corners.
pub fn segmented_timeout_selector(
    ui: &mut egui::Ui,
    current_minutes: &mut u32,
    test_seconds: bool,
) -> bool {
    let mut changed = false;
    let timeouts = IdleTimeout::ALL;
    let num_segments = timeouts.len() as f32;
    let gap = SPACING_XS;
    let total_width = ui.available_width();
    let segment_width = ((total_width - gap * (num_segments - 1.0)) / num_segments).max(10.0);
    let segment_height = 24.0;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;

        for &timeout in &timeouts {
            let mins = timeout.as_minutes();
            let is_selected = *current_minutes == mins;
            let label = if test_seconds {
                match timeout {
                    IdleTimeout::FiveMinutes => "5s",
                    IdleTimeout::FifteenMinutes => "15s",
                    IdleTimeout::ThirtyMinutes => "30s",
                    IdleTimeout::OneHour => "60s",
                    IdleTimeout::TwoHours => "120s",
                }
            } else {
                match timeout {
                    IdleTimeout::FiveMinutes => "5m",
                    IdleTimeout::FifteenMinutes => "15m",
                    IdleTimeout::ThirtyMinutes => "30m",
                    IdleTimeout::OneHour => "1h",
                    IdleTimeout::TwoHours => "2h",
                }
            };

            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(segment_width, segment_height), Sense::click());

            if response.clicked() && !is_selected {
                *current_minutes = mins;
                changed = true;
            }

            // Background fill
            let bg_color = if is_selected {
                COLOR_ACCENT
            } else if response.is_pointer_button_down_on() {
                Color32::from_rgba_unmultiplied(216, 210, 199, 45)
            } else if response.hovered() {
                Color32::from_rgba_unmultiplied(216, 210, 199, 20)
            } else {
                Color32::from_rgba_unmultiplied(16, 15, 18, 120)
            };

            // Border stroke
            let stroke = if is_selected {
                Stroke::NONE
            } else if response.hovered() {
                Stroke::new(1.0, COLOR_ACCENT)
            } else {
                Stroke::new(1.0, CARD_BORDER)
            };

            ui.painter()
                .rect_filled(rect, CORNER_RADIUS_SHARP, bg_color);
            if stroke != Stroke::NONE {
                ui.painter()
                    .rect_stroke(rect, CORNER_RADIUS_SHARP, stroke, StrokeKind::Inside);
            }

            // Text label
            let text_color = if is_selected {
                COLOR_SURFACE
            } else if response.hovered() {
                COLOR_TEXT
            } else {
                COLOR_MUTED
            };

            let galley = ui.painter().layout_no_wrap(
                label.to_string(),
                FontId::proportional(11.0),
                text_color,
            );
            let text_pos = egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            );
            ui.painter().galley(text_pos, galley, text_color);
        }
    });

    changed
}

/// Custom flat mod-menu toggle switch row with hover highlight.
pub fn flat_toggle_row(ui: &mut egui::Ui, label: &str, value: &mut bool) -> Response {
    let row_height = 26.0;
    let switch_width = 28.0;
    let switch_height = 14.0;
    let thumb_size = 8.0;

    let (row_rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), row_height), Sense::click());

    if response.clicked() {
        *value = !*value;
    }

    // Row hover highlight
    if response.hovered() {
        ui.painter().rect_filled(
            row_rect,
            CORNER_RADIUS_SHARP,
            Color32::from_rgba_unmultiplied(216, 210, 199, 12),
        );
    }

    // Label on left
    let text_color = if *value { COLOR_TEXT } else { COLOR_MUTED };
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::proportional(FONT_SIZE_BODY),
        text_color,
    );
    let text_pos = egui::pos2(
        row_rect.min.x + 4.0,
        row_rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, text_color);

    // Switch capsule on right
    let switch_rect = Rect::from_min_size(
        egui::pos2(
            row_rect.max.x - switch_width - 4.0,
            row_rect.center().y - switch_height / 2.0,
        ),
        Vec2::new(switch_width, switch_height),
    );

    let (switch_bg, thumb_x, thumb_color) = if *value {
        (
            COLOR_ACCENT,
            switch_rect.max.x - thumb_size - 3.0,
            COLOR_SURFACE,
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(20, 19, 22, 180),
            switch_rect.min.x + 3.0,
            COLOR_MUTED,
        )
    };

    ui.painter()
        .rect_filled(switch_rect, CORNER_RADIUS_SHARP, switch_bg);

    if !*value {
        ui.painter().rect_stroke(
            switch_rect,
            CORNER_RADIUS_SHARP,
            Stroke::new(1.0, CARD_BORDER),
            StrokeKind::Inside,
        );
    }

    let thumb_rect = Rect::from_min_size(
        egui::pos2(thumb_x, switch_rect.center().y - thumb_size / 2.0),
        Vec2::new(thumb_size, thumb_size),
    );
    ui.painter()
        .rect_filled(thumb_rect, CORNER_RADIUS_SHARP, thumb_color);

    response
}

/// Compact live telemetry readouts for "IDLE" and "SHUTDOWN IN" timers.
pub fn live_telemetry_readouts(
    ui: &mut egui::Ui,
    idle_secs: u32,
    countdown_secs: Option<u32>,
    enabled: bool,
) {
    let gap = SPACING_XS;
    let card_w = ((ui.available_width() - gap) / 2.0).max(10.0);
    let card_h = 44.0;

    let idle_str = format_countdown(idle_secs);
    let shutdown_str = if enabled {
        countdown_secs.map_or_else(|| "--:--".to_string(), format_countdown)
    } else {
        "PAUSED".to_string()
    };

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;

        // Card 1: IDLE FOR
        render_telemetry_card(ui, card_w, card_h, "IDLE FOR", &idle_str, enabled);

        // Card 2: SHUTDOWN IN
        render_telemetry_card(ui, card_w, card_h, "SHUTDOWN IN", &shutdown_str, enabled);
    });
}

fn render_telemetry_card(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    label: &str,
    value: &str,
    active: bool,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());

    ui.painter().rect_filled(
        rect,
        CORNER_RADIUS_SHARP,
        Color32::from_rgba_unmultiplied(14, 13, 16, 160),
    );
    ui.painter().rect_stroke(
        rect,
        CORNER_RADIUS_SHARP,
        Stroke::new(1.0, CARD_BORDER),
        StrokeKind::Inside,
    );

    // Muted top label
    let label_galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::monospace(FONT_SIZE_READOUT_LABEL),
        COLOR_MUTED,
    );
    let label_pos = egui::pos2(rect.min.x + 6.0, rect.min.y + 4.0);
    ui.painter().galley(label_pos, label_galley, COLOR_MUTED);

    // Large numeric readout
    let value_color = if active { COLOR_ACCENT } else { COLOR_MUTED };
    let val_galley = ui.painter().layout_no_wrap(
        value.to_string(),
        FontId::monospace(FONT_SIZE_READOUT),
        value_color,
    );
    let val_pos = egui::pos2(rect.min.x + 6.0, rect.min.y + 18.0);
    ui.painter().galley(val_pos, val_galley, value_color);
}

/// Mod menu style outlined button.
pub fn mod_button(ui: &mut egui::Ui, label: &str) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());

    let (bg, border, text_color) = if response.is_pointer_button_down_on() {
        (
            Color32::from_rgba_unmultiplied(216, 210, 199, 40),
            COLOR_ACCENT,
            COLOR_TEXT,
        )
    } else if response.hovered() {
        (
            Color32::from_rgba_unmultiplied(216, 210, 199, 20),
            COLOR_ACCENT,
            COLOR_TEXT,
        )
    } else {
        (
            Color32::from_rgba_unmultiplied(16, 15, 18, 140),
            CARD_BORDER,
            COLOR_MUTED,
        )
    };

    ui.painter().rect_filled(rect, CORNER_RADIUS_SHARP, bg);
    ui.painter().rect_stroke(
        rect,
        CORNER_RADIUS_SHARP,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );

    let galley =
        ui.painter()
            .layout_no_wrap(label.to_string(), FontId::proportional(11.0), text_color);
    let text_pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, text_color);

    response.clicked()
}

/// Hairline divider separating content sections.
pub fn hairline_divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().line_segment(
        [
            egui::pos2(rect.min.x, rect.center().y),
            egui::pos2(rect.max.x, rect.center().y),
        ],
        Stroke::new(1.0, HAIRLINE_DIVIDER),
    );
}
