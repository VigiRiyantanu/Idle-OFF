//! Modular UI architecture for the IdleOff settings window.
//!
//! Submodules:
//! - [`theme`]: Mod-menu color palette, typography scale, Inter font, and layout spacing constants.
//! - [`background`]: Embedded PNG decoding, GPU texture caching, and stretched background painter.
//! - [`widgets`]: Custom flat toggles, sharp segmented timeout selector, and telemetry cards.
//! - [`header`]: Frameless draggable header strip with custom minimize and close buttons.
//! - [`sidebar`]: Narrow left sidebar navigation tabs ("Main", "Popup", "System", "About").
//! - [`tabs`]: Content panel views for each individual tab.
//! - [`settings_view`]: Application state machine and master mod-menu layout.

pub mod background;
pub mod header;
pub mod settings_view;
pub mod sidebar;
pub mod tabs;
pub mod theme;
pub mod widgets;

use crate::logger;
use crate::util::to_wide;
use eframe::egui::{self, Vec2};
use settings_view::SettingsApp;
use std::ptr::null_mut;
use std::sync::Arc;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, FALSE};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetSystemMetrics, SetForegroundWindow, ShowWindow, SM_CXSCREEN, SM_CYSCREEN,
    SW_RESTORE,
};

/// Generates the embedded 32x32 application icon for the egui viewport taskbar.
fn load_app_icon() -> Option<egui::IconData> {
    let width = 32u32;
    let height = 32u32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    let center_x = 15.5f32;
    let center_y = 15.5f32;
    let radius = 13.5f32;

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= radius {
                let is_top_gap = dy < 0.0 && dx.abs() < 3.0;
                let is_arc = (6.0..=8.5).contains(&dist) && !is_top_gap;
                let is_stem = dx.abs() <= 1.2 && (-10.0..=-2.0).contains(&dy);

                if is_arc || is_stem {
                    rgba.extend_from_slice(&[0x00, 0xF0, 0xFF, 0xFF]);
                } else {
                    rgba.extend_from_slice(&[0x1E, 0x29, 0x3B, 0xFF]);
                }
            } else {
                rgba.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
            }
        }
    }

    Some(egui::IconData {
        rgba,
        width,
        height,
    })
}

/// Launches the settings window UI, ensuring a single instance and proper foreground focus.
pub fn run_settings_window(test_seconds: bool) {
    let mutex_name = to_wide("Local\\IdleOff_Settings_Mutex_9F8D2E");
    // SAFETY: Checking settings window single instance mutex.
    let mutex_handle = unsafe { CreateMutexW(null_mut(), FALSE, mutex_name.as_ptr()) };

    if mutex_handle.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        if !mutex_handle.is_null() {
            unsafe { CloseHandle(mutex_handle) };
        }
        // If settings window is already open, restore and bring it to front
        let window_title = to_wide("IdleOff");
        let mut existing = unsafe { FindWindowW(null_mut(), window_title.as_ptr()) };
        if existing.is_null() {
            let alt_title = to_wide("IdleOff - Settings");
            existing = unsafe { FindWindowW(null_mut(), alt_title.as_ptr()) };
        }

        if !existing.is_null() {
            logger::log("Existing settings window detected; restoring to foreground");
            unsafe {
                ShowWindow(existing, SW_RESTORE);
                SetForegroundWindow(existing);
            }
        }
        return;
    }

    // Calculate window centered coordinates on primary screen (compact 420x300 mod-menu size)
    let window_w = 420.0;
    let window_h = 300.0;
    let (center_x, center_y) = unsafe {
        let screen_w = GetSystemMetrics(SM_CXSCREEN) as f32;
        let screen_h = GetSystemMetrics(SM_CYSCREEN) as f32;
        (
            ((screen_w - window_w) / 2.0).max(0.0),
            ((screen_h - window_h) / 2.0).max(0.0),
        )
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(Vec2::new(window_w, window_h))
        .with_position(egui::pos2(center_x, center_y))
        .with_resizable(false)
        .with_maximize_button(false)
        .with_decorations(false)
        .with_title("IdleOff");

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(Arc::new(icon));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "IdleOff",
        native_options,
        Box::new(move |cc| {
            // Configure Inter typography and dark cohesive palette
            theme::configure_fonts(&cc.egui_ctx);
            theme::configure_style(&cc.egui_ctx);

            Ok(Box::new(SettingsApp::new(test_seconds)))
        }),
    );

    unsafe {
        CloseHandle(mutex_handle);
    }
}
