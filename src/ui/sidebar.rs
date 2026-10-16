//! Left sidebar with mod-menu navigation tabs.
//!
//! Complies strictly with specifications:
//! - Narrow left sidebar with 4 tabs: "Main", "Popup", "System", "About".
//! - Hover highlight on rows.
//! - Active tab marked by an accent bar on the left edge.
//! - Small monospace / condensed uppercase font.

use crate::ui::theme::{
    CARD_BORDER, COLOR_ACCENT, COLOR_MUTED, COLOR_TEXT, FONT_SIZE_TAB, SIDEBAR_FILL, SPACING_XS,
};
use eframe::egui::{self, Color32, FontId, Rect, Sense, Stroke, Vec2};

pub const SIDEBAR_WIDTH: f32 = 118.0;

/// Available tabs in the mod-menu settings interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Main,
    Popup,
    System,
    About,
}

impl SettingsTab {
    pub const ALL: [Self; 4] = [Self::Main, Self::Popup, Self::System, Self::About];

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Main => "MAIN",
            Self::Popup => "POPUP",
            Self::System => "SYSTEM",
            Self::About => "ABOUT",
        }
    }
}

/// Renders the narrow left sidebar with tabs.
pub fn render_sidebar(ui: &mut egui::Ui, current_tab: &mut SettingsTab, daemon_enabled: bool) {
    let height = ui.available_height();
    let (sidebar_rect, _) =
        ui.allocate_exact_size(Vec2::new(SIDEBAR_WIDTH, height), Sense::hover());

    // Sidebar background
    ui.painter().rect_filled(sidebar_rect, 0.0, SIDEBAR_FILL);

    // Right border divider
    ui.painter().line_segment(
        [
            egui::pos2(sidebar_rect.max.x, sidebar_rect.min.y),
            egui::pos2(sidebar_rect.max.x, sidebar_rect.max.y),
        ],
        Stroke::new(1.0, CARD_BORDER),
    );

    let tab_height = 34.0;
    let mut top_y = sidebar_rect.min.y + 6.0;

    for tab in SettingsTab::ALL {
        let is_active = *current_tab == tab;
        let tab_rect = Rect::from_min_size(
            egui::pos2(sidebar_rect.min.x, top_y),
            Vec2::new(SIDEBAR_WIDTH, tab_height),
        );

        let response = ui.interact(
            tab_rect,
            ui.scope_id().with(("tab_select", tab.label())),
            Sense::click(),
        );

        if response.clicked() {
            *current_tab = tab;
        }

        // Active vs Hover background
        if is_active {
            ui.painter().rect_filled(
                tab_rect,
                0.0,
                Color32::from_rgba_unmultiplied(216, 210, 199, 25),
            );

            // Left vertical accent bar (2.5 px width)
            let bar_rect = Rect::from_min_size(tab_rect.min, Vec2::new(2.5, tab_height));
            ui.painter().rect_filled(bar_rect, 0.0, COLOR_ACCENT);
        } else if response.hovered() {
            ui.painter().rect_filled(
                tab_rect,
                0.0,
                Color32::from_rgba_unmultiplied(216, 210, 199, 12),
            );
        }

        // Tab label
        let text_color = if is_active {
            COLOR_TEXT
        } else if response.hovered() {
            COLOR_ACCENT
        } else {
            COLOR_MUTED
        };

        let galley = ui.painter().layout_no_wrap(
            tab.label().to_string(),
            FontId::monospace(FONT_SIZE_TAB),
            text_color,
        );
        let text_pos = egui::pos2(
            tab_rect.min.x + 12.0,
            tab_rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(text_pos, galley, text_color);

        top_y += tab_height + 2.0;
    }

    // Bottom daemon status indicator
    let footer_h = 24.0;
    let footer_rect = Rect::from_min_size(
        egui::pos2(
            sidebar_rect.min.x,
            sidebar_rect.max.y - footer_h - SPACING_XS,
        ),
        Vec2::new(SIDEBAR_WIDTH, footer_h),
    );

    let status_str = if daemon_enabled {
        "[ACTIVE]"
    } else {
        "[PAUSED]"
    };
    let status_color = if daemon_enabled {
        COLOR_ACCENT
    } else {
        COLOR_MUTED
    };

    let status_galley = ui.painter().layout_no_wrap(
        status_str.to_string(),
        FontId::monospace(11.5),
        status_color,
    );
    let status_pos = egui::pos2(
        footer_rect.min.x + 12.0,
        footer_rect.center().y - status_galley.size().y / 2.0,
    );
    ui.painter().galley(status_pos, status_galley, status_color);
}
