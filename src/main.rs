//! IdleOff - Lightweight native Windows auto-shutdown desktop utility.
//!
//! Runs silently in the system tray, monitors user idle time, shows an optional
//! countdown warning, and gracefully shuts down the system after a configurable timeout.

#![windows_subsystem = "windows"]

mod app;
mod autostart;
mod idle;
mod logger;
mod popup;
mod settings;
mod settings_ui;
mod shutdown;
mod tray;
mod ui;
mod util;

use app::{Action, App, MenuAction};
use idle::get_idle_seconds;
use popup::{PopupWindow, IDC_CANCEL_SHUTDOWN, IDC_SHUTDOWN_NOW};
use std::cell::RefCell;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use tray::{TrayIcon, NIN_KEYSELECT, NIN_SELECT, TRAY_ICON_UID, WM_TRAYICON};
use util::to_wide;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, FALSE, HANDLE, HINSTANCE,
    HWND, LPARAM, LRESULT, WAIT_TIMEOUT, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::{
    CreateMutexW, OpenProcess, TerminateProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE,
};
use windows_sys::Win32::UI::Shell::{Shell_NotifyIconW, NIM_DELETE, NOTIFYICONDATAW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    FindWindowW, GetMessageW, GetWindowThreadProcessId, KillTimer, PostMessageW, PostQuitMessage,
    RegisterClassExW, RegisterWindowMessageW, SetForegroundWindow, SetTimer, ShowWindow,
    TranslateMessage, ASFW_ANY, CW_USEDEFAULT, MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND,
    SW_RESTORE, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
    WM_POWERBROADCAST, WM_RBUTTONUP, WM_TIMER, WM_WTSSESSION_CHANGE, WNDCLASSEXW,
    WTS_SESSION_LOGON, WTS_SESSION_UNLOCK,
};

/// Low-frequency idle polling timer ID.
const TIMER_ID_POLL: usize = 1;
/// 1-second countdown timer ID active during the warning popup.
const TIMER_ID_COUNTDOWN: usize = 2;

/// Production idle polling interval: 5000 ms (5 seconds).
///
/// Justification:
/// The minimum user-configurable timeout is 5 minutes (300 seconds). A 5-second polling interval
/// yields at most a 1.6% variance on idle detection while consuming virtually 0% CPU.
/// The message loop thread wakes up once every 5 seconds, performs a sub-microsecond
/// `GetLastInputInfo` check, and immediately returns to sleep in `GetMessageW`.
const POLL_INTERVAL_MS: u32 = 5000;

/// Rapid polling interval used only in test mode (`--test-seconds`): 1000 ms.
const TEST_POLL_INTERVAL_MS: u32 = 1000;

/// Application runtime context owned on the main UI thread.
struct MainContext {
    app: App,
    tray: TrayIcon,
    popup: PopupWindow,
    taskbar_created_msg: u32,
    config_changed_msg: u32,
    test_popup_msg: u32,
    instant_shutdown_msg: u32,
    last_tooltip: String,
}

thread_local! {
    static CONTEXT: RefCell<Option<MainContext>> = const { RefCell::new(None) };
}

/// Dispatches actions returned by the state machine to UI elements.
fn dispatch_action(hwnd: HWND, ctx: &mut MainContext, action: Action) {
    match action {
        Action::None => {}
        Action::ShowWarning { remaining_seconds } => {
            logger::log(&format!(
                "Showing warning popup (remaining={remaining_seconds}s)"
            ));
            ctx.popup.show(remaining_seconds);
            // SAFETY: Start high-resolution 1-second timer to update countdown label.
            unsafe {
                SetTimer(hwnd, TIMER_ID_COUNTDOWN, 1000, None);
            }
        }
        Action::UpdateWarning { remaining_seconds } => {
            ctx.popup.update_countdown(remaining_seconds);
        }
        Action::CloseWarning => {
            logger::log("Closing warning popup");
            ctx.popup.hide();
            // SAFETY: Stop 1-second countdown timer and resume resting low-frequency polling.
            unsafe {
                KillTimer(hwnd, TIMER_ID_COUNTDOWN);
            }
        }
        Action::ExecuteShutdown => {
            logger::log("Action::ExecuteShutdown triggered");
            // SAFETY: Clean up timers and hide UI prior to shutdown.
            unsafe {
                KillTimer(hwnd, TIMER_ID_POLL);
                KillTimer(hwnd, TIMER_ID_COUNTDOWN);
            }
            ctx.popup.hide();
            let _ = shutdown::execute_shutdown(ctx.app.dry_run);
        }
    }
}

/// Updates the tray icon hover tooltip only when the text actually changes.
fn sync_tooltip(ctx: &mut MainContext) {
    let current = ctx.app.tooltip_text();
    if current != ctx.last_tooltip {
        ctx.tray.update_tooltip(&current);
        ctx.last_tooltip = current;
    }
}

/// Global flag set immediately when exit begins to prevent new actions or timers.
static EXITING: AtomicBool = AtomicBool::new(false);
/// Tracks the last spawn timestamp in milliseconds to debounce rapid duplicate launches.
static LAST_SPAWN_TICKS: AtomicU64 = AtomicU64::new(0);
/// Stores the background settings child process PID if currently spawned.
static SETTINGS_CHILD_PID: AtomicU32 = AtomicU32::new(0);
/// Stores the single-instance mutex handle so request_exit can release it immediately.
static DAEMON_MUTEX: AtomicIsize = AtomicIsize::new(0);

/// Opens the egui settings window or brings it to the front if already open.
fn open_settings_window() {
    logger::log("Opening settings window");

    // SAFETY: Grant permission to the settings window to take foreground focus.
    unsafe {
        AllowSetForegroundWindow(ASFW_ANY);
    }

    let settings_title = to_wide("IdleOff");
    let mut existing = unsafe { FindWindowW(null_mut(), settings_title.as_ptr()) };
    if existing.is_null() {
        let legacy_title = to_wide("IdleOff - Settings");
        existing = unsafe { FindWindowW(null_mut(), legacy_title.as_ptr()) };
    }

    if !existing.is_null() {
        logger::log("Existing settings window found; restoring to foreground");
        // SAFETY: Restore from minimized state if needed and bring window to front.
        unsafe {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
        }
        return;
    }

    // Debounce rapid duplicate launches (e.g. from double-clicking the tray icon)
    let now = unsafe { GetTickCount64() };
    let last = LAST_SPAWN_TICKS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < 800 {
        logger::log("Ignoring rapid duplicate settings window spawn request");
        return;
    }
    LAST_SPAWN_TICKS.store(now, Ordering::Relaxed);

    if let Ok(exe_path) = std::env::current_exe() {
        let mut cmd = std::process::Command::new(exe_path);
        cmd.arg("--settings");
        if logger::is_logging_enabled() {
            cmd.arg("--log");
        }
        if std::env::args().any(|a| a == "--test-seconds") {
            cmd.arg("--test-seconds");
        }
        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id();
                SETTINGS_CHILD_PID.store(pid, Ordering::SeqCst);
                logger::log(&format!("Spawned settings window PID={pid}"));
                // SAFETY: Specifically grant foreground window permission to the child process.
                unsafe {
                    AllowSetForegroundWindow(pid);
                }
            }
            Err(e) => {
                logger::log(&format!("Failed to spawn settings window: {e}"));
            }
        }
    }
}

/// Closes or terminates the settings window child process if active.
fn terminate_settings_child_process() {
    let settings_title = to_wide("IdleOff");
    let existing_wnd = unsafe { FindWindowW(null_mut(), settings_title.as_ptr()) };
    let mut target_pid = SETTINGS_CHILD_PID.load(Ordering::SeqCst);

    if !existing_wnd.is_null() {
        let mut window_pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(existing_wnd, &mut window_pid);
            PostMessageW(existing_wnd, WM_CLOSE, 0, 0);
        }
        if window_pid != 0 {
            target_pid = window_pid;
        }
    }

    if target_pid != 0 {
        // SAFETY: Open process with PROCESS_TERMINATE | SYNCHRONIZE to await exit or terminate.
        let proc_handle =
            unsafe { OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, FALSE, target_pid) };

        if !proc_handle.is_null() {
            // SAFETY: Wait up to 500ms for graceful exit via WM_CLOSE.
            let wait_res = unsafe { WaitForSingleObject(proc_handle, 500) };
            if wait_res == WAIT_TIMEOUT {
                logger::log(&format!(
                    "Settings child PID={target_pid} did not exit within 500ms; terminating forcefully"
                ));
                // SAFETY: Force-terminate lingering settings process.
                unsafe {
                    TerminateProcess(proc_handle, 0);
                }
            } else {
                logger::log(&format!(
                    "Settings child PID={target_pid} terminated cleanly"
                ));
            }
            unsafe {
                CloseHandle(proc_handle);
            }
        }
    }
}

/// Unified graceful exit sequence for the IdleOff daemon.
///
/// Guaranteed to run on the main UI thread and perform cleanup in strict order:
/// 1. Set `EXITING` flag.
/// 2. Close warning popup.
/// 3. Kill all timers.
/// 4. Terminate settings child process (max 500ms wait).
/// 5. Remove tray icon (NIM_DELETE).
/// 6. Release single-instance mutex.
/// 7. DestroyWindow(hwnd) & PostQuitMessage(0).
/// 8. Watchdog safety net exits after 2 seconds if anything hangs.
fn request_exit(hwnd: HWND) {
    if EXITING.swap(true, Ordering::SeqCst) {
        return;
    }
    logger::log("request_exit sequence initiated");

    // Watchdog safety net: terminate process after 2000ms if message loop hangs
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(2000));
        logger::log("Exit watchdog timer expired (2000ms); calling std::process::exit(0)");
        std::process::exit(0);
    });

    // 2. Close warning popup window & 6. Remove tray icon
    CONTEXT.with(|cell| {
        if let Ok(mut borrow) = cell.try_borrow_mut() {
            if let Some(mut ctx) = borrow.take() {
                logger::log("Closing warning popup and deleting tray icon");
                ctx.popup.hide();
                ctx.popup.destroy();
                ctx.tray.delete();
                // SAFETY: Unregistering desktop session change notifications.
                unsafe {
                    WTSUnRegisterSessionNotification(hwnd);
                }
            }
        }
    });

    // Fallback tray delete in case context was already taken
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_UID;
    // SAFETY: Explicitly remove tray icon to avoid ghost icon.
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }

    // 3. Kill timers
    // SAFETY: Killing all active Win32 timers.
    unsafe {
        KillTimer(hwnd, TIMER_ID_POLL);
        KillTimer(hwnd, TIMER_ID_COUNTDOWN);
    }

    // 4. Terminate the settings child process
    terminate_settings_child_process();

    // 7. Release single-instance mutex handle
    let mutex_isize = DAEMON_MUTEX.swap(0, Ordering::SeqCst);
    if mutex_isize != 0 {
        logger::log("Releasing single-instance mutex handle");
        // SAFETY: Closing mutex handle to allow immediate clean restart.
        unsafe {
            CloseHandle(mutex_isize as HANDLE);
        }
    }

    // 8. DestroyWindow(hwnd)
    logger::log("Destroying daemon hidden receiver window");
    // SAFETY: Destroying the receiver window.
    unsafe {
        DestroyWindow(hwnd);
        // 9. PostQuitMessage(0) to ensure GetMessageW immediately unblocks with 0.
        PostQuitMessage(0);
    }
}

/// Dispatches high-level menu actions returned from `show_context_menu`.
fn handle_menu_action(hwnd: HWND, action: MenuAction) {
    logger::log(&format!("Handling menu action: {action:?}"));
    match action {
        MenuAction::None => {}
        MenuAction::OpenSettings => {
            open_settings_window();
        }
        MenuAction::Exit => {
            logger::log("Exit selected from tray context menu");
            request_exit(hwnd);
        }
    }
}

/// Window procedure for the hidden message-receiver window.
unsafe extern "system" fn main_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // 9. In the window procedure: WM_DESTROY => { PostQuitMessage(0); return 0; }
    if msg == WM_DESTROY {
        logger::log("WM_DESTROY received in main_wnd_proc; posting quit message");
        PostQuitMessage(0);
        return 0;
    }

    // Ignore all other events once exiting is initiated
    if EXITING.load(Ordering::Relaxed) {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    CONTEXT.with(|cell| {
        let Ok(mut borrow) = cell.try_borrow_mut() else {
            // Re-entrant invocation during modal loops (e.g. TrackPopupMenuEx):
            // Fall back cleanly to default window procedure to avoid panicking.
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        };
        let ctx = match borrow.as_mut() {
            Some(c) => c,
            None => return DefWindowProcW(hwnd, msg, wparam, lparam),
        };

        // 1. Windows Explorer restart notification
        if msg == ctx.taskbar_created_msg {
            ctx.tray.re_add();
            return 0;
        }

        // 2. Settings window modified configuration notification
        if msg == ctx.config_changed_msg {
            logger::log("Received config_changed_msg from settings window");
            let new_settings = settings::load();
            if ctx.app.matches_settings(&new_settings) {
                logger::log("Settings unchanged; ignoring reload");
                return 0;
            }
            let action = ctx.app.apply_settings(&new_settings);
            dispatch_action(hwnd, ctx, action);
            sync_tooltip(ctx);
            return 0;
        }

        // 3. Settings window requested a 10s test warning popup
        if msg == ctx.test_popup_msg {
            logger::log("Received test_popup_msg from settings window");
            dispatch_action(
                hwnd,
                ctx,
                Action::ShowWarning {
                    remaining_seconds: 10,
                },
            );
            return 0;
        }

        // 4. Settings window requested instant shutdown
        if msg == ctx.instant_shutdown_msg {
            logger::log("Received instant_shutdown_msg from settings window");
            dispatch_action(hwnd, ctx, Action::ExecuteShutdown);
            return 0;
        }

        match msg {
            WM_CLOSE => {
                // The hidden main window ignores WM_CLOSE.
                // The daemon only terminates when the user chooses "Exit" from the tray context menu.
                logger::log("WM_CLOSE received on hidden receiver window; ignoring");
                0
            }

            WM_TIMER => {
                let idle_secs = get_idle_seconds().unwrap_or(0);
                let action = ctx.app.on_tick(idle_secs);
                dispatch_action(hwnd, ctx, action);
                sync_tooltip(ctx);
                0
            }

            WM_TRAYICON => {
                let event = (lparam & 0xFFFF) as u32;
                let anchor_x = (wparam as u32 & 0xFFFF) as i16 as i32;
                let anchor_y = ((wparam as u32 >> 16) & 0xFFFF) as i16 as i32;

                logger::log(&format!(
                    "WM_TRAYICON received: event=0x{event:04X} at ({anchor_x}, {anchor_y})"
                ));

                match event {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT | NIN_KEYSELECT => {
                        // Left-click, double-click, or keyboard select opens/restores settings window
                        open_settings_window();
                    }
                    WM_CONTEXTMENU | WM_RBUTTONUP => {
                        // Right-click or context menu key displays minimal standard context menu
                        let action = ctx.tray.show_context_menu(hwnd, anchor_x, anchor_y);
                        if action == MenuAction::Exit {
                            // Drop RefCell borrow before triggering exit sequence to avoid re-entrancy
                            drop(borrow);
                            request_exit(hwnd);
                            return 0;
                        }
                        handle_menu_action(hwnd, action);
                    }
                    _ => {}
                }
                0
            }

            WM_COMMAND => {
                let cmd_id = (wparam & 0xFFFF) as u32;
                logger::log(&format!("WM_COMMAND received: id={cmd_id}"));
                match cmd_id {
                    IDC_CANCEL_SHUTDOWN => {
                        let action = ctx.app.on_cancel_warning();
                        dispatch_action(hwnd, ctx, action);
                    }
                    IDC_SHUTDOWN_NOW => {
                        dispatch_action(hwnd, ctx, Action::ExecuteShutdown);
                    }
                    _ => {}
                }
                0
            }

            WM_POWERBROADCAST => {
                if wparam == PBT_APMRESUMEAUTOMATIC as usize
                    || wparam == PBT_APMRESUMESUSPEND as usize
                {
                    let action = ctx.app.on_system_resume_or_unlock();
                    dispatch_action(hwnd, ctx, action);
                }
                0
            }

            WM_WTSSESSION_CHANGE => {
                if wparam == WTS_SESSION_UNLOCK as usize || wparam == WTS_SESSION_LOGON as usize {
                    let action = ctx.app.on_system_resume_or_unlock();
                    dispatch_action(hwnd, ctx, action);
                }
                0
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has_arg = |flag: &str| args.iter().any(|a| a == flag);

    if has_arg("--log") {
        logger::set_logging_enabled(true);
        logger::log("================ IdleOff Started ================");
    }

    let test_seconds = has_arg("--test-seconds") || std::env::var("IDLEOFF_TEST_SECONDS").is_ok();
    let dry_run = has_arg("--dry-run") || std::env::var("IDLEOFF_DRY_RUN").is_ok();

    // Mode Dispatch: If launched with --settings, run the egui settings UI directly
    if has_arg("--settings") {
        logger::log("Starting in --settings UI mode");
        settings_ui::run_settings_window(test_seconds);
        return;
    }

    // 1. Single Instance Check: ensure only one daemon instance of IdleOff can ever run.
    let mutex_name = to_wide("Global\\IdleOff_Daemon_Mutex_9F8D2E");
    // SAFETY: Creating a named mutex in the global namespace.
    let mutex_handle = unsafe { CreateMutexW(null_mut(), FALSE, mutex_name.as_ptr()) };
    let last_err = unsafe { GetLastError() };

    let daemon_class_name = to_wide("IdleOff_Daemon_Class");
    let existing_window = unsafe { FindWindowW(daemon_class_name.as_ptr(), null_mut()) };

    let is_already_running = mutex_handle.is_null()
        || last_err == ERROR_ALREADY_EXISTS
        || last_err == ERROR_ACCESS_DENIED
        || !existing_window.is_null();

    if is_already_running {
        if !mutex_handle.is_null() {
            unsafe { CloseHandle(mutex_handle) };
        }

        // Open the settings window if launched interactively
        let is_silent = has_arg("--silent") || has_arg("--quiet");
        if !is_silent {
            open_settings_window();
        }
        return;
    }

    DAEMON_MUTEX.store(mutex_handle as isize, Ordering::SeqCst);

    // 3. Load persistent settings
    let settings = settings::load();
    let app = App::new(&settings, test_seconds, dry_run);
    let initial_tooltip = app.tooltip_text();

    // 4. Register hidden message receiver window class
    let hinstance: HINSTANCE = unsafe { GetModuleHandleW(null_mut()) };

    // SAFETY: Registering the hidden window class.
    unsafe {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(main_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null_mut(),
            lpszClassName: daemon_class_name.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&wc);
    }

    // 5. Create hidden message window
    let window_title = to_wide("IdleOff_HiddenReceiver");
    // SAFETY: Creating an invisible message-handling window.
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            daemon_class_name.as_ptr(),
            window_title.as_ptr(),
            0,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null_mut(),
        )
    };

    if hwnd.is_null() {
        logger::log("Failed to create daemon hidden window");
        let handle = DAEMON_MUTEX.swap(0, Ordering::SeqCst);
        if handle != 0 {
            unsafe { CloseHandle(handle as HANDLE) };
        }
        return;
    }

    // 6. Register for session notification (lock/unlock)
    // SAFETY: Registering for desktop session change notifications.
    unsafe {
        WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
    }

    // 7. Register window messages
    let taskbar_str = to_wide("TaskbarCreated");
    let config_changed_str = to_wide("IdleOff_ConfigChanged");
    let test_popup_str = to_wide("IdleOff_TestPopup");
    let instant_shutdown_str = to_wide("IdleOff_InstantShutdown");

    let taskbar_created_msg = unsafe { RegisterWindowMessageW(taskbar_str.as_ptr()) };
    let config_changed_msg = unsafe { RegisterWindowMessageW(config_changed_str.as_ptr()) };
    let test_popup_msg = unsafe { RegisterWindowMessageW(test_popup_str.as_ptr()) };
    let instant_shutdown_msg = unsafe { RegisterWindowMessageW(instant_shutdown_str.as_ptr()) };

    // 8. Initialize System Tray and Warning Popup
    let tray = TrayIcon::new(hwnd, hinstance, &initial_tooltip);
    let popup = match PopupWindow::new(hinstance, hwnd) {
        Ok(p) => p,
        Err(e) => {
            logger::log(&format!("Failed to create popup window: {e}"));
            let handle = DAEMON_MUTEX.swap(0, Ordering::SeqCst);
            if handle != 0 {
                unsafe { CloseHandle(handle as HANDLE) };
            }
            return;
        }
    };

    let poll_interval = if test_seconds {
        TEST_POLL_INTERVAL_MS
    } else {
        POLL_INTERVAL_MS
    };

    CONTEXT.with(|cell| {
        *cell.borrow_mut() = Some(MainContext {
            app,
            tray,
            popup,
            taskbar_created_msg,
            config_changed_msg,
            test_popup_msg,
            instant_shutdown_msg,
            last_tooltip: initial_tooltip,
        });
    });

    // 9. Start initial polling timer
    // SAFETY: Setting the Win32 idle polling timer.
    unsafe {
        SetTimer(hwnd, TIMER_ID_POLL, poll_interval, None);
    }

    logger::log("Daemon message pump started");

    // 10. Native Win32 message loop: single-threaded, sleeping between events
    // SAFETY: Standard Windows message pump.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let handle = DAEMON_MUTEX.swap(0, Ordering::SeqCst);
        if handle != 0 {
            CloseHandle(handle as HANDLE);
        }
    }

    logger::log("IdleOff Daemon exiting cleanly");
}
