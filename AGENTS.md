# AGENTS.md — Instructions for Autonomous & Coding Agents

This document defines repository instructions, architectural constraints, safety guidelines, and conventions for AI and human pair programmers working on **IdleOff**.

---

## 🎯 Project Mission

IdleOff is a minimal-footprint, native Windows desktop background utility built in Rust that auto-shuts down the computer after a configurable idle time.

---

## 📐 Invariants & Technical Rules

1. **Native Win32 First**:
   - The resident background daemon must remain pure Win32 using `windows-sys`.
   - Never load `egui`, `eframe`, `winit`, `tokio`, or heavy runtimes inside the daemon loop.
   - The settings window is isolated in child mode (`--settings`) via `src/settings_ui.rs`.
2. **Zero-Panic Policy**:
   - The release profile uses `panic = "abort"`.
   - Never use `.unwrap()`, `.expect()`, or slice indexings in code paths reachable from window procedures (`main_wnd_proc`, `popup_wnd_proc`) or timer callbacks.
   - Wrap error-prone logic in `Result` or defensive matches with fallback defaults.
3. **Win32 Menu Dispatch**:
   - Always use `TrackPopupMenuEx` with `TPM_RETURNCMD`.
   - Never assume menu IDs are received through `WM_COMMAND`.
   - Command ID `0` signifies menu dismissal and must strictly map to `MenuAction::None`.
4. **Tick Math & Wrap-Around**:
   - The Win32 tick counter wraps around every ~49.7 days of continuous system uptime.
   - Always use `current_ticks.wrapping_sub(last_input_ticks)` for elapsed millisecond calculations.
5. **Atomic Settings Persistence**:
   - Always save `%TEMP%\idle-off\settings.json` using atomic temp-file write followed by rename.
   - If corrupted or missing, always fall back gracefully to default settings (`enabled=true`, `timeout=30`, `popup_enabled=true`, `start_with_windows=false`).
6. **Autostart Sync**:
   - When checking or displaying autostart state, query `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` directly rather than trusting cached flags.

---

## 🛠️ Verification & Test Commands

Every pull request or modification must satisfy the following checks with zero errors and zero warnings:

```powershell
# 1. Run complete unit test suite
cargo test

# 2. Verify strict Clippy standards (no warnings permitted)
cargo clippy -- -D warnings

# 3. Check code formatting
cargo fmt --check

# 4. Verify release build compilation
cargo build --release
```

---

## 📁 Module Responsibilities

| File | Primary Responsibility |
|---|---|
| `src/main.rs` | Flag parsing, mode dispatch (`--settings` vs daemon), Win32 event pump, single instance mutex. |
| `src/app.rs` | State machine enum, tick evaluation logic, timeout definitions, menu action mapping. |
| `src/idle.rs` | `GetLastInputInfo` wrappers and wrapping subtraction calculations. |
| `src/tray.rs` | Shell notification icon, tooltip updates, and context menu construction. |
| `src/popup.rs` | Always-on-top, non-activating native countdown warning window. |
| `src/settings_ui.rs` | `eframe` / `egui` control panel for configuration and telemetry. |
| `src/settings.rs` | Zero-dependency handwritten JSON parser and atomic disk storage (`%TEMP%\idle-off\settings.json`). |
| `src/autostart.rs` | Windows Registry Run key reading and writing. |
| `src/shutdown.rs` | `SE_SHUTDOWN_NAME` privilege escalation and `ExitWindowsEx` execution. |
| `src/logger.rs` | Non-panicking timestamped file logger for diagnostic mode (`--log`). |
| `src/util.rs` | Wide-string conversion and formatting helpers. |
| `build.rs` | Generates embedded power icon (`app.ico`), `app.manifest`, and compiles `app.rc`. |
