//! System tray (notification area) icon and context menu management.
//!
//! Complies strictly with Windows Shell NOTIFYICON_VERSION_4 specifications:
//! - Upgrades icon to NOTIFYICON_VERSION_4 via NIM_SETVERSION.
//! - Retries creation on daemon startup if Windows Explorer is not yet ready.
//! - Re-registers cleanly on Explorer restart (TaskbarCreated).
//! - Context menu strictly contains "Open IdleOff" (default bold) and "Exit".
//! - Uses TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_NONOTIFY with WM_NULL dismissal fix.

use crate::app::{map_menu_id, MenuAction, IDM_EXIT, IDM_OPEN_SETTINGS};
use crate::logger;
use crate::util::{copy_to_wide_buffer, to_wide};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, POINT};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, PostMessageW,
    SetForegroundWindow, SetMenuDefaultItem, TrackPopupMenuEx, IDI_APPLICATION, MF_SEPARATOR,
    MF_STRING, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_NULL,
};

/// Custom window message sent by the shell for tray icon notifications.
pub const WM_TRAYICON: u32 = WM_APP + 1;

/// Shell keyboard selection notification codes for NOTIFYICON_VERSION_4.
pub const NIN_SELECT: u32 = 1024;
pub const NIN_KEYSELECT: u32 = 1025;

pub const TRAY_ICON_UID: u32 = 100;
const IDI_APP_ICON: u16 = 1;

/// Tray icon wrapper holding the Win32 `NOTIFYICONDATAW` structure.
pub struct TrayIcon {
    nid: NOTIFYICONDATAW,
}

impl TrayIcon {
    /// Creates and registers the taskbar notification icon using NOTIFYICON_VERSION_4.
    ///
    /// If initial registration fails (e.g. Explorer is still initializing at user login),
    /// retries up to 5 times with a 500ms delay.
    pub fn new(hwnd: HWND, hinstance: HINSTANCE, initial_tooltip: &str) -> Self {
        let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ICON_UID;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAYICON;

        // Try loading the embedded icon from resources; fall back to standard application icon
        // SAFETY: Loading application icon from module resources or standard icon.
        let hicon = unsafe {
            let res_icon = LoadIconW(hinstance, IDI_APP_ICON as *const u16);
            if !res_icon.is_null() {
                res_icon
            } else {
                LoadIconW(null_mut(), IDI_APPLICATION)
            }
        };
        nid.hIcon = hicon;

        copy_to_wide_buffer(initial_tooltip, &mut nid.szTip);

        // Attempt registration with retry logic for login/startup delays
        let mut added = false;
        for attempt in 1..=5 {
            // SAFETY: Registering the tray icon with NIM_ADD.
            let ret = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
            if ret != 0 {
                added = true;
                logger::log(&format!(
                    "Tray icon successfully registered on attempt {attempt}"
                ));
                break;
            }
            logger::log(&format!(
                "Tray icon NIM_ADD failed on attempt {attempt}; retrying in 500ms..."
            ));
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        if added {
            // Upgrade to NOTIFYICON_VERSION_4
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            // SAFETY: Setting NOTIFYICON_VERSION_4 behavior on registered icon.
            let ver_ret = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &nid) };
            if ver_ret != 0 {
                logger::log("Tray icon upgraded to NOTIFYICON_VERSION_4");
            } else {
                logger::log("Warning: Shell_NotifyIconW(NIM_SETVERSION) returned 0");
            }
        } else {
            logger::log("Error: Failed to register tray icon after 5 attempts");
        }

        Self { nid }
    }

    /// Re-adds the tray icon (e.g. after Windows Explorer restarts via TaskbarCreated).
    pub fn re_add(&mut self) {
        logger::log("Re-adding tray icon on TaskbarCreated notification");
        let mut added = false;
        for _attempt in 1..=5 {
            // SAFETY: Re-registering tray icon with NIM_ADD.
            let ret = unsafe { Shell_NotifyIconW(NIM_ADD, &self.nid) };
            if ret != 0 {
                added = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        if added {
            self.nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            // SAFETY: Re-setting NOTIFYICON_VERSION_4 on re-added icon.
            unsafe {
                Shell_NotifyIconW(NIM_SETVERSION, &self.nid);
            }
            logger::log("Tray icon re-added and set to NOTIFYICON_VERSION_4 successfully");
        }
    }

    /// Updates the hover tooltip text only.
    pub fn update_tooltip(&mut self, tooltip: &str) {
        copy_to_wide_buffer(tooltip, &mut self.nid.szTip);
        self.nid.uFlags = NIF_TIP;
        // SAFETY: Modifying only the tooltip flag and buffer of the existing tray icon.
        unsafe {
            Shell_NotifyIconW(NIM_MODIFY, &self.nid);
        }
        self.nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    }

    /// Displays the right-click tray context menu at specified anchor coordinates.
    ///
    /// Context menu items strictly adhere to standard Windows specifications:
    /// 1. "Open IdleOff" (bold default item)
    /// 2. Separator
    /// 3. "Exit"
    ///
    /// Returns the selected [`MenuAction`] directly via `TPM_RETURNCMD`.
    pub fn show_context_menu(&self, hwnd: HWND, anchor_x: i32, anchor_y: i32) -> MenuAction {
        logger::log(&format!(
            "Displaying tray context menu at anchor ({anchor_x}, {anchor_y})"
        ));

        // SAFETY: Setting foreground window before displaying popup menu ensures
        // the menu properly dismisses when clicking elsewhere.
        unsafe {
            SetForegroundWindow(hwnd);

            let hmenu = CreatePopupMenu();
            if hmenu.is_null() {
                logger::log("Error: CreatePopupMenu returned null handle");
                return MenuAction::None;
            }

            // 1. "Open IdleOff" (default bold action)
            let open_str = to_wide("Open IdleOff");
            AppendMenuW(
                hmenu,
                MF_STRING,
                IDM_OPEN_SETTINGS as usize,
                open_str.as_ptr(),
            );

            // 2. Separator
            AppendMenuW(hmenu, MF_SEPARATOR, 0, null_mut());

            // 3. "Exit"
            let exit_str = to_wide("Exit");
            AppendMenuW(hmenu, MF_STRING, IDM_EXIT as usize, exit_str.as_ptr());

            // Mark "Open IdleOff" as the default bold menu item
            SetMenuDefaultItem(hmenu, IDM_OPEN_SETTINGS, 0);

            // Prefer physical cursor position to ensure menu appears at mouse position
            let mut pt = POINT { x: 0, y: 0 };
            GetCursorPos(&mut pt);
            let (x, y) = if pt.x != 0 || pt.y != 0 {
                (pt.x, pt.y)
            } else if anchor_x != 0 || anchor_y != 0 {
                (anchor_x, anchor_y)
            } else {
                (0, 0)
            };

            // Display popup menu using TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_NONOTIFY
            let chosen_cmd = TrackPopupMenuEx(
                hmenu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_NONOTIFY,
                x,
                y,
                hwnd,
                null_mut(),
            );

            // Win32 standard fix for tray menu dismissal
            PostMessageW(hwnd, WM_NULL, 0, 0);

            // Free GDI menu resources immediately
            DestroyMenu(hmenu);

            logger::log(&format!("Context menu selected command: {chosen_cmd}"));
            map_menu_id(chosen_cmd as u32)
        }
    }

    /// Removes the tray icon cleanly on application exit.
    pub fn delete(&mut self) {
        // SAFETY: Cleanly unregistering the tray icon with NIM_DELETE.
        unsafe {
            Shell_NotifyIconW(NIM_DELETE, &self.nid);
        }
    }
}

impl Drop for TrayIcon {
    fn drop(&mut self) {
        self.delete();
    }
}
