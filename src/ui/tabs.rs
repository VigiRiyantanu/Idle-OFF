//! Tab content rendering functions for the IdleOff settings window.
//!
//! Sub-views:
//! - [`render_main_tab`]: Enabled toggle, timeout segmented selector, and live telemetry.
//! - [`render_popup_tab`]: Cancel popup toggle and test popup button.
//! - [`render_system_tab`]: Start with Windows registry toggle.
//! - [`render_about_tab`]: Application version, architecture specs, and tray info.

use crate::config::Config;
use crate::ui::theme::{
    COLOR_ACCENT, COLOR_MUTED, COLOR_TEXT, FONT_SIZE_BODY, SPACING_MD, SPACING_SM, SPACING_XS,
};
use crate::ui::widgets::{
    flat_toggle_row, hairline_divider, live_telemetry_readouts, mod_button, section_header,
    segmented_timeout_selector,
};
use eframe::egui::{self, FontId};

/// Renders the "Main" tab view.
pub fn render_main_tab(
    ui: &mut egui::Ui,
    config: &mut Config,
    test_seconds: bool,
    idle_secs: u32,
) -> bool {
    let mut changed = false;

    // 1. Live Telemetry Readouts
    section_header(ui, "LIVE TELEMETRY");
    ui.add_space(SPACING_XS);

    let timeout_secs = if test_seconds {
        config.timeout_minutes
    } else {
        config.timeout_minutes * 60
    };
    let countdown = if idle_secs >= timeout_secs {
        Some(0)
    } else {
        Some(timeout_secs.saturating_sub(idle_secs))
    };

    live_telemetry_readouts(ui, idle_secs, countdown, config.enabled);

    ui.add_space(SPACING_SM);
    hairline_divider(ui);
    ui.add_space(SPACING_SM);

    // 2. Idle Timeout Selector
    section_header(ui, "IDLE TIMEOUT THRESHOLD");
    ui.add_space(SPACING_XS);

    if segmented_timeout_selector(ui, &mut config.timeout_minutes, test_seconds) {
        changed = true;
    }

    ui.add_space(SPACING_SM);
    hairline_divider(ui);
    ui.add_space(SPACING_SM);

    // 3. Enabled Toggle
    section_header(ui, "SERVICE STATE");
    ui.add_space(SPACING_XS);

    if flat_toggle_row(ui, "Idle Monitoring Enabled", &mut config.enabled).changed() {
        changed = true;
    }

    changed
}

/// Renders the "Popup" tab view.
pub fn render_popup_tab(
    ui: &mut egui::Ui,
    config: &mut Config,
    test_popup_clicked: &mut bool,
) -> bool {
    let mut changed = false;

    section_header(ui, "WARNING POPUP SETTINGS");
    ui.add_space(SPACING_XS);

    if flat_toggle_row(
        ui,
        "Cancel popup 3 min before shutdown",
        &mut config.show_popup,
    )
    .changed()
    {
        changed = true;
    }

    ui.add_space(SPACING_XS);
    ui.label(
        egui::RichText::new("Displays a native always-on-top countdown window allowing instant cancellation before shutdown occurs.")
            .font(FontId::proportional(11.0))
            .color(COLOR_MUTED),
    );

    ui.add_space(SPACING_MD);
    hairline_divider(ui);
    ui.add_space(SPACING_MD);

    section_header(ui, "DIAGNOSTIC TEST");
    ui.add_space(SPACING_XS);

    if mod_button(ui, "TEST WARNING POPUP (10s)") {
        *test_popup_clicked = true;
    }

    changed
}

/// Renders the "System" tab view.
pub fn render_system_tab(ui: &mut egui::Ui, autostart: &mut bool) -> bool {
    let mut changed = false;

    section_header(ui, "SYSTEM INTEGRATION");
    ui.add_space(SPACING_XS);

    if flat_toggle_row(ui, "Start with Windows", autostart).changed() {
        changed = true;
    }

    ui.add_space(SPACING_XS);
    ui.label(
        egui::RichText::new("Registers IdleOff in HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run for automatic background startup at login.")
            .font(FontId::proportional(11.0))
            .color(COLOR_MUTED),
    );

    ui.add_space(SPACING_MD);
    hairline_divider(ui);
    ui.add_space(SPACING_MD);

    section_header(ui, "DAEMON ARCHITECTURE");
    ui.add_space(SPACING_XS);
    ui.label(
        egui::RichText::new(
            "Status: Resident pure Win32 daemon running in system tray with zero GUI overhead.",
        )
        .font(FontId::proportional(11.0))
        .color(COLOR_MUTED),
    );

    changed
}

/// Renders the "About" tab view.
pub fn render_about_tab(ui: &mut egui::Ui) {
    section_header(ui, "APPLICATION SPECIFICATION");
    ui.add_space(SPACING_XS);

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("IDLEOFF")
                .font(FontId::monospace(13.0))
                .color(COLOR_ACCENT)
                .strong(),
        );
        ui.label(
            egui::RichText::new("v0.1.0")
                .font(FontId::monospace(11.0))
                .color(COLOR_MUTED),
        );
    });

    ui.add_space(SPACING_XS);
    ui.label(
        egui::RichText::new("Lightweight native Windows auto-shutdown desktop utility.")
            .font(FontId::proportional(FONT_SIZE_BODY))
            .color(COLOR_TEXT),
    );

    ui.add_space(SPACING_SM);
    hairline_divider(ui);
    ui.add_space(SPACING_SM);

    section_header(ui, "CONTROLS & TRAY");
    ui.add_space(SPACING_XS);
    ui.label(
        egui::RichText::new("• Drag top bar to move window.\n• [Esc] or [✕] closes settings window (daemon stays running in tray).\n• To terminate IdleOff completely, right-click tray icon and choose Exit.")
            .font(FontId::proportional(11.0))
            .color(COLOR_MUTED),
    );
}
