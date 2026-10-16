//! Always-on-top countdown warning popup window.
//!
//! Uses a lightweight native Win32 window (`WS_EX_TOPMOST | WS_EX_TOOLWINDOW`) shown with
//! `SW_SHOWNOACTIVATE` so that it never aggressively steals focus or intercepts keyboard input.

use crate::util::{format_countdown, to_wide};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetStockObject, UpdateWindow, DEFAULT_GUI_FONT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, LoadCursorW, PostMessageW,
    RegisterClassExW, SendMessageW, SetWindowTextW, ShowWindow, CS_HREDRAW, CS_VREDRAW, IDC_ARROW,
    SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOWNOACTIVATE, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_SETFONT, WNDCLASSEXW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_POPUP, WS_SYSMENU, WS_VISIBLE,
};

pub const IDC_CANCEL_SHUTDOWN: u32 = 3001;
pub const IDC_SHUTDOWN_NOW: u32 = 3002;
const IDC_COUNTDOWN_LABEL: u32 = 3003;

const SS_CENTER: u32 = 0x00000001;

/// Popup window state and child control handles.
pub struct PopupWindow {
    pub hwnd: HWND,
    h_countdown: HWND,
}

// Stores the main window handle to forward button events
static mut G_MAIN_HWND: HWND = null_mut();

/// Window procedure for the warning popup dialog.
unsafe extern "system" fn popup_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => 0,
        WM_COMMAND => {
            let cmd_id = (wparam & 0xFFFF) as u32;
            if cmd_id == IDC_CANCEL_SHUTDOWN || cmd_id == IDC_SHUTDOWN_NOW {
                let main_hwnd = G_MAIN_HWND;
                if !main_hwnd.is_null() {
                    PostMessageW(main_hwnd, WM_COMMAND, cmd_id as WPARAM, 0);
                }
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        WM_CLOSE => {
            // Closing via 'X' button or Alt+F4 cancels the pending shutdown
            let main_hwnd = G_MAIN_HWND;
            if !main_hwnd.is_null() {
                PostMessageW(main_hwnd, WM_COMMAND, IDC_CANCEL_SHUTDOWN as WPARAM, 0);
            }
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

impl PopupWindow {
    /// Registers the popup window class and creates child controls.
    pub fn new(hinstance: HINSTANCE, main_hwnd: HWND) -> Result<Self, &'static str> {
        let class_name = to_wide("IdleOff_PopupClass");

        // SAFETY: Registering the popup window class.
        unsafe {
            G_MAIN_HWND = main_hwnd;

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(popup_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                hbrBackground: (windows_sys::Win32::Graphics::Gdi::COLOR_BTNFACE + 1) as _,
                lpszMenuName: null_mut(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: null_mut(),
            };
            RegisterClassExW(&wc);
        }

        let width = 360;
        let height = 180;

        // Position window centered on primary monitor
        let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        let x = (screen_w - width) / 2;
        let y = (screen_h - height) / 2;

        let title = to_wide("IdleOff - Auto Shutdown Warning");
        let ex_style = WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
        let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_BORDER;

        // SAFETY: Creating the top-level warning dialog window.
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                width,
                height,
                null_mut(),
                null_mut(),
                hinstance,
                null_mut(),
            )
        };

        if hwnd.is_null() {
            return Err("Failed to create warning popup window");
        }

        let static_class = to_wide("STATIC");
        let button_class = to_wide("BUTTON");
        let default_font = unsafe { GetStockObject(DEFAULT_GUI_FONT) };

        // 1. Informative description label
        let prompt_text = to_wide("Your computer has been idle. Auto shutdown in:");
        let h_prompt = unsafe {
            CreateWindowExW(
                0,
                static_class.as_ptr(),
                prompt_text.as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_CENTER,
                20,
                20,
                320,
                24,
                hwnd,
                null_mut(),
                hinstance,
                null_mut(),
            )
        };

        // 2. Countdown display (large text)
        let countdown_text = to_wide("03:00");
        let h_countdown = unsafe {
            CreateWindowExW(
                0,
                static_class.as_ptr(),
                countdown_text.as_ptr(),
                WS_CHILD | WS_VISIBLE | SS_CENTER,
                20,
                52,
                320,
                30,
                hwnd,
                IDC_COUNTDOWN_LABEL as _,
                hinstance,
                null_mut(),
            )
        };

        // 3. "Cancel" button
        let cancel_text = to_wide("Cancel");
        let h_cancel = unsafe {
            CreateWindowExW(
                0,
                button_class.as_ptr(),
                cancel_text.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                65,
                98,
                105,
                32,
                hwnd,
                IDC_CANCEL_SHUTDOWN as _,
                hinstance,
                null_mut(),
            )
        };

        // 4. "Shutdown Now" button
        let shutdown_text = to_wide("Shutdown Now");
        let h_shutdown = unsafe {
            CreateWindowExW(
                0,
                button_class.as_ptr(),
                shutdown_text.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                190,
                98,
                105,
                32,
                hwnd,
                IDC_SHUTDOWN_NOW as _,
                hinstance,
                null_mut(),
            )
        };

        // Apply clean system font to all controls
        unsafe {
            SendMessageW(h_prompt, WM_SETFONT, default_font as WPARAM, 1);
            SendMessageW(h_countdown, WM_SETFONT, default_font as WPARAM, 1);
            SendMessageW(h_cancel, WM_SETFONT, default_font as WPARAM, 1);
            SendMessageW(h_shutdown, WM_SETFONT, default_font as WPARAM, 1);
        }

        Ok(Self { hwnd, h_countdown })
    }

    /// Displays the popup window without taking keyboard focus.
    pub fn show(&self, remaining_seconds: u32) {
        self.update_countdown(remaining_seconds);
        // SAFETY: Showing without activation prevents accidental keypress interception.
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            UpdateWindow(self.hwnd);
        }
    }

    /// Updates the displayed countdown timer string ("mm:ss").
    pub fn update_countdown(&self, remaining_seconds: u32) {
        let text = format_countdown(remaining_seconds);
        let wide_text = to_wide(&text);
        // SAFETY: Updating label text safely.
        unsafe {
            SetWindowTextW(self.h_countdown, wide_text.as_ptr());
        }
    }

    /// Hides the popup window.
    pub fn hide(&self) {
        // SAFETY: Hiding the window.
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Destroys the window handle.
    pub fn destroy(&mut self) {
        if !self.hwnd.is_null() {
            // SAFETY: Destroying popup window handle on shutdown.
            unsafe {
                DestroyWindow(self.hwnd);
            }
            self.hwnd = null_mut();
        }
    }
}
