//! Custom frameless header strip with window drag area and custom titlebar buttons.
//!
//! Complies strictly with specifications:
//! - Window drag interaction via `ui.interact(...)` + `ViewportCommand::StartDrag`.
//! - App name on the left in small uppercase typography.
//! - Flat painted minimize (`—`) and close (`✕`) button glyphs (no emoji).
//! - Close ends the settings process only; daemon remains unaffected.

use crate::ui::theme::{
    COLOR_ACCENT, COLOR_MUTED, COLOR_TEXT, FONT_SIZE_HEADER_TITLE, HEADER_FILL,
};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, Vec2, ViewportCommand};

pub const HEADER_HEIGHT: f32 = 26.0;
const BUTTON_WIDTH: f32 = 28.0;

/// Renders the custom window titlebar header strip.
pub fn render_header(ui: &mut egui::Ui) {
    let (header_rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), HEADER_HEIGHT),
        Sense::hover(),
    );

    // Header background fill
    ui.painter().rect_filled(header_rect, 0.0, HEADER_FILL);

    // Bottom 1 px divider
    ui.painter().line_segment(
        [
            egui::pos2(header_rect.min.x, header_rect.max.y),
            egui::pos2(header_rect.max.x, header_rect.max.y),
        ],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(216, 210, 199, 30)),
    );

    let buttons_total_width = BUTTON_WIDTH * 2.0;
    let drag_rect = Rect::from_min_max(
        header_rect.min,
        egui::pos2(header_rect.max.x - buttons_total_width, header_rect.max.y),
    );

    // Drag interaction over non-button header area
    let drag_resp = ui.interact(drag_rect, ui.scope_id().with("header_drag"), Sense::drag());
    if drag_resp.drag_started() {
        ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
    }

    // App name on the left (small uppercase monospace/condensed)
    let title_galley = ui.painter().layout_no_wrap(
        "IDLEOFF // SETTINGS".to_string(),
        FontId::monospace(FONT_SIZE_HEADER_TITLE),
        COLOR_ACCENT,
    );
    let title_pos = egui::pos2(
        header_rect.min.x + 10.0,
        header_rect.center().y - title_galley.size().y / 2.0,
    );
    ui.painter().galley(title_pos, title_galley, COLOR_ACCENT);

    // Minimize Button
    let min_rect = Rect::from_min_size(
        egui::pos2(header_rect.max.x - buttons_total_width, header_rect.min.y),
        Vec2::new(BUTTON_WIDTH, HEADER_HEIGHT),
    );
    let min_resp = ui.interact(min_rect, ui.scope_id().with("header_min"), Sense::click());
    if min_resp.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
    if min_resp.hovered() {
        ui.painter().rect_filled(
            min_rect,
            0.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 20),
        );
    }
    // Draw horizontal minimize glyph
    let min_stroke = Stroke::new(
        1.2,
        if min_resp.hovered() {
            COLOR_TEXT
        } else {
            COLOR_MUTED
        },
    );
    let min_y = min_rect.center().y + 3.0;
    ui.painter().line_segment(
        [
            egui::pos2(min_rect.center().x - 4.5, min_y),
            egui::pos2(min_rect.center().x + 4.5, min_y),
        ],
        min_stroke,
    );

    // Close Button
    let close_rect = Rect::from_min_size(
        egui::pos2(header_rect.max.x - BUTTON_WIDTH, header_rect.min.y),
        Vec2::new(BUTTON_WIDTH, HEADER_HEIGHT),
    );
    let close_resp = ui.interact(
        close_rect,
        ui.scope_id().with("header_close"),
        Sense::click(),
    );
    if close_resp.clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }
    if close_resp.hovered() {
        ui.painter()
            .rect_filled(close_rect, 0.0, Color32::from_rgb(0xA0, 0x22, 0x22));
    }
    // Draw cross 'X' close glyph
    let close_stroke = Stroke::new(
        1.2,
        if close_resp.hovered() {
            Color32::WHITE
        } else {
            COLOR_MUTED
        },
    );
    let offset = 3.5;
    let center = close_rect.center();
    ui.painter().line_segment(
        [
            egui::pos2(center.x - offset, center.y - offset),
            egui::pos2(center.x + offset, center.y + offset),
        ],
        close_stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(center.x - offset, center.y + offset),
            egui::pos2(center.x + offset, center.y - offset),
        ],
        close_stroke,
    );
}
