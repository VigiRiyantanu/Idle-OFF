# IdleOff Architecture

This document outlines the architectural design, module layout, execution models, and inter-process communication (IPC) of **IdleOff**.

---

## 1. Architectural Philosophy

1. **Near-Zero Resident Overhead**: The primary daemon process runs 24/7 in the user's system tray. It must never load heavy GUI runtimes or background worker threads when resting.
2. **Deterministic State Machine**: State transitions (`Paused`, `Monitoring`, `Warning`, `ShuttingDown`) are modeled as pure functions and separated completely from Win32 API calls to facilitate automated unit testing.
3. **Reentrancy and Panic Safety**: Win32 window callbacks run synchronously on the message thread. No reentrant mutable borrows, unwraps, or panics are allowed in the window procedure.
4. **Non-Intrusive UX**: Warning dialogs must never steal keyboard focus or interrupt active user input.

---

## 2. Process Model (Two-Mode Single Binary)

IdleOff utilizes a **Two-Mode Architecture** bundled into a single binary (`idleoff.exe`):

```
                        [ User Launch ]
                              │
                              ▼
                       main() in main.rs
                              │
                   Has `--settings` flag?
                    /                \
                 YES                  NO
                  │                    │
                  ▼                    ▼
      [ Settings UI Mode ]    [ Daemon Mode (Default) ]
      - eframe / egui         - Named Mutex Check
      - Connects to Daemon    - Win32 Message Loop
      - Saves config.toml     - System Tray Icon
      - Sends IPC message     - Native Warning Popup
      - Terminates on (X)     - Privilege & Shutdown
```

### Mode Comparison

| Attribute | Daemon Mode | Settings UI Mode |
|---|---|---|
| **Invocation** | `idleoff.exe` | `idleoff.exe --settings` |
| **GUI Framework** | None (Raw Win32 User32/GDI) | `eframe` / `egui` (Glow/OpenGL) |
| **Idle RAM** | ~4 - 5 MB | ~30 MB (only while open) |
| **CPU Usage** | 0.0% | Active only during render |
| **Lifetime** | Continuous background resident | Ephemeral (terminates on close) |
| **Mutex Guard** | `Global\IdleOff_Daemon_Mutex_9F8D2E` | `Local\IdleOff_Settings_Mutex_9F8D2E` |

---

## 3. Module Hierarchy

```
src/
├── main.rs         # Entry point, flag parser, mode dispatcher, Win32 message loop
├── app.rs          # State machine, timeout logic, menu-action mapping, pure rules
├── idle.rs         # Win32 GetLastInputInfo + tick difference with wrap-around
├── tray.rs         # Taskbar notification icon, TrackPopupMenuEx (TPM_RETURNCMD)
├── popup.rs        # Non-activating Win32 warning dialog with countdown timer
├── settings_ui.rs  # Eframe / egui control panel with live telemetry & IPC
├── config.rs       # Handwritten TOML parser, atomic file persistence (temp + rename)
├── autostart.rs    # Windows Registry HKCU\Software\...\Run manager
├── shutdown.rs     # Privilege escalation (SE_SHUTDOWN_NAME) & ExitWindowsEx call
├── logger.rs       # Diagnostic file logger active when --log is passed
└── util.rs         # UTF-16 wide string conversions, countdown formatting
```

---

## 4. State Machine Design

The application state machine is modeled as an explicit enum:

```rust
pub enum AppState {
    Paused,
    Monitoring,
    Warning { remaining_seconds: u32 },
    ShuttingDown,
}
```

### State Transitions

```
                    ┌──────────────┐
                    │    Paused    │
                    └──────┬───────┘
                           │ Toggle Enabled
                           ▼
                    ┌──────────────┐
       ┌───────────►│  Monitoring  │◄───────────┐
       │            └──────┬───────┘            │
       │                   │                    │
       │                   │ Idle >= (Timeout - 3m)
       │ User Input        │ [and Popup Enabled]
       │ or Cancel         ▼                    │
       │            ┌──────────────┐            │ User Input
       └────────────┤   Warning    ├────────────┘ or Cancel
                    └──────┬───────┘
                           │ Countdown reaches 0
                           ▼
                    ┌──────────────┐
                    │ ShuttingDown │
                    └──────────────┘
```

### Pure Transition Evaluation
The function `evaluate_tick()` takes the current state, observed idle seconds, timeout, and popup preferences, returning the next state and an [`Action`]:

- `Action::None`: No change needed.
- `Action::ShowWarning { remaining_seconds }`: Display the native popup window and start 1s countdown timer.
- `Action::UpdateWarning { remaining_seconds }`: Update countdown text.
- `Action::CloseWarning`: Hide popup window and kill 1s countdown timer.
- `Action::ExecuteShutdown`: Terminate timers, hide UI, and execute system shutdown.

---

## 5. Inter-Process Communication (IPC)

The Settings child process communicates with the Daemon resident process via Windows **Registered Window Messages**:

1. **`IdleOff_ConfigChanged`**:
   - Sent when any setting (timeout, popup toggle, enabled state) is modified in the Settings UI.
   - The daemon reloads `%APPDATA%\IdleOff\config.toml`, synchronizes its internal state, and refreshes the tray tooltip.
2. **`IdleOff_TestPopup`**:
   - Sent when the user clicks the "Test Warning Popup (10s)" button in the Settings UI.
   - The daemon immediately invokes `Action::ShowWarning { remaining_seconds: 10 }`, allowing the user to inspect the native dialog, verify the countdown, and test button/mouse cancellation.

---

## 6. Win32 Menu Reentrancy Elimination

Win32's standard `TrackPopupMenu` runs a nested synchronous message pump on the calling thread. If called while holding an exclusive `RefCell::borrow_mut()`, any message dispatched during menu interaction triggers a reentrant borrow panic.

To ensure 100% stability:
1. `src/tray.rs` invokes `TrackPopupMenuEx` with `TPM_RETURNCMD`.
2. Windows returns the selected menu item ID directly to the function instead of posting a `WM_COMMAND`.
3. If the user dismisses the menu, ID `0` is returned.
4. `map_menu_id()` guarantees that `0` and unknown IDs map strictly to `MenuAction::None`, never triggering unexpected actions or exit.
