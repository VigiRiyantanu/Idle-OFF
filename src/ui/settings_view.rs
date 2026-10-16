//! Layout and presentation logic for the IdleOff Settings window.
//!
//! Orchestrates the custom header strip, left sidebar tab navigation, background layer,
//! and tab content views in a compact mod-menu layout.

use crate::autostart::set_autostart_enabled;
use crate::idle::get_idle_seconds;
use crate::settings::{self, Settings};
use crate::ui::background::BackgroundLayer;
use crate::ui::header::render_header;
use crate::ui::sidebar::{render_sidebar, SettingsTab};
use crate::ui::tabs::{render_about_tab, render_main_tab, render_popup_tab, render_system_tab};
use crate::ui::theme::{BORDER_ACCENT, CORNER_RADIUS_SHARP, OVERLAY_FILL, SPACING_MD, SPACING_SM};
use crate::util::to_wide;
use eframe::egui::{self, Frame, Margin, Stroke, StrokeKind, ViewportCommand};
use std::ptr::null_mut;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, PostMessageW, RegisterWindowMessageW,
};

/// Graphical settings application state.
pub struct SettingsApp {
    settings: Settings,
    test_seconds: bool,
    current_tab: SettingsTab,
    daemon_hwnd: isize,
    msg_config_changed: u32,
    msg_test_popup: u32,
    background: BackgroundLayer,
}

impl SettingsApp {
    /// Creates a new settings application instance.
    pub fn new(test_seconds: bool) -> Self {
        let settings = settings::load();

        let daemon_class = to_wide("IdleOff_Daemon_Class");
        // SAFETY: Finding existing daemon receiver window to establish IPC message connection.
        let daemon_hwnd = unsafe { FindWindowW(daemon_class.as_ptr(), null_mut()) as isize };

        let str_config_changed = to_wide("IdleOff_ConfigChanged");
        let str_test_popup = to_wide("IdleOff_TestPopup");

        // SAFETY: Registering custom window messages for daemon synchronization.
        let msg_config_changed = unsafe { RegisterWindowMessageW(str_config_changed.as_ptr()) };
        let msg_test_popup = unsafe { RegisterWindowMessageW(str_test_popup.as_ptr()) };

        Self {
            settings,
            test_seconds,
            current_tab: SettingsTab::Main,
            daemon_hwnd,
            msg_config_changed,
            msg_test_popup,
            background: BackgroundLayer::default(),
        }
    }

    /// Persists configuration to disk and notifies the background daemon.
    fn save_and_notify(&self) {
        let _ = settings::save(&self.settings);
        if self.daemon_hwnd != 0 {
            // SAFETY: Notifying daemon window via PostMessageW that configuration changed.
            unsafe {
                PostMessageW(self.daemon_hwnd as _, self.msg_config_changed, 0, 0);
            }
        }
    }

    /// Notifies the daemon to trigger a 10-second test warning dialog.
    fn trigger_test_popup(&self) {
        if self.daemon_hwnd != 0 {
            // SAFETY: Posting test popup message to daemon receiver window.
            unsafe {
                PostMessageW(self.daemon_hwnd as _, self.msg_test_popup, 0, 0);
            }
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Redraw only once per second so the window consumes practically 0% CPU
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));

        // [Esc] closes the settings window without touching the daemon
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.ctx().send_viewport_cmd(ViewportCommand::Close);
            return;
        }

        // 1. Paint background image stretched to the entire window rect
        self.background.paint(ui);

        // 2. Full-window dark semi-transparent glass panel with sharp accent border
        let full_rect = ui.max_rect();
        ui.painter()
            .rect_filled(full_rect, CORNER_RADIUS_SHARP, OVERLAY_FILL);
        ui.painter().rect_stroke(
            full_rect,
            CORNER_RADIUS_SHARP,
            Stroke::new(1.0, BORDER_ACCENT),
            StrokeKind::Inside,
        );

        // 3. Frameless Header Strip with Drag interaction and Minimize/Close buttons
        render_header(ui);

        let mut settings_dirty = false;
        let mut autostart_dirty = false;
        let mut trigger_test = false;

        // 4. Mod-Menu Body Layout: Left Sidebar + Right Content Area
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;

            // Sidebar navigation
            render_sidebar(ui, &mut self.current_tab, self.settings.enabled);

            // Right tab content area
            ui.vertical(|ui| {
                ui.add_space(SPACING_SM);
                Frame {
                    inner_margin: Margin::same(SPACING_MD as i8),
                    ..Default::default()
                }
                .show(ui, |ui| {
                    let current_idle_secs = get_idle_seconds().unwrap_or(0);
                    match self.current_tab {
                        SettingsTab::Main => {
                            if render_main_tab(
                                ui,
                                &mut self.settings,
                                self.test_seconds,
                                current_idle_secs,
                            ) {
                                settings_dirty = true;
                            }
                        }
                        SettingsTab::Popup => {
                            if render_popup_tab(ui, &mut self.settings, &mut trigger_test) {
                                settings_dirty = true;
                            }
                        }
                        SettingsTab::System => {
                            if render_system_tab(ui, &mut self.settings.start_with_windows) {
                                autostart_dirty = true;
                            }
                        }
                        SettingsTab::About => {
                            render_about_tab(ui);
                        }
                    }
                });
            });
        });

        if trigger_test {
            self.trigger_test_popup();
        }

        if autostart_dirty {
            let _ = set_autostart_enabled(self.settings.start_with_windows);
            self.save_and_notify();
        } else if settings_dirty {
            self.save_and_notify();
        }
    }
}
