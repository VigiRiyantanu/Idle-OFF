# IdleOff ⏱️💻

A high-performance, ultra-lightweight Windows desktop utility written in Rust. **IdleOff** runs silently in the background notification area (system tray) and automatically executes a clean system shutdown when the computer has been idle for a user-configured duration.

---

## ✨ Features

- **System-Wide Idle Detection**: Tracks global keyboard and mouse inactivity using native Win32 `GetLastInputInfo` and `GetTickCount` with 32-bit tick counter wrap-around handling.
- **Ultra-Low Resource Footprint**: 
  - Resident background daemon consumes **0% CPU** and **~4 MB RAM**.
  - Single-threaded message pump (`GetMessageW`) sleeps between events.
  - Low-frequency polling timer (`SetTimer` at 5s in idle state, switching to 1s only during active countdown).
- **Selectable Timeouts**:
  - `5 minutes`
  - `15 minutes`
  - `30 minutes` (default)
  - `1 hour`
  - `2 hours`
- **Optional Countdown Warning Popup**:
  - Displays a non-activating, always-on-top native Win32 dialog (`WS_EX_TOPMOST | WS_EX_TOOLWINDOW`, shown with `SW_SHOWNOACTIVATE`) 3 minutes before shutdown.
  - *5-minute option*: Popup appears at 2 minutes of idle time (giving exactly 3 minutes warning before shutdown at minute 5).
  - Live `mm:ss` countdown timer.
  - "Cancel" button to abort shutdown and reset the idle cycle.
  - **Auto-abort on input**: Moving the mouse or pressing any keyboard key during the countdown automatically dismisses the warning and resets the idle cycle.
- **Native System Tray**:
  - Right-click context menu (rebuilt dynamically from current state):
    1. Status indicator (disabled grey item): `Status: Monitoring | Shutdown after 30 min idle`
    2. `[✓] Enabled` toggle
    3. `Idle timeout` submenu with radio-style checkmarks (`5m`, `15m`, `30m`, `1h`, `2h`)
    4. `[✓] Show cancel popup (3 min before shutdown)`
    5. `[✓] Start with Windows` (synced directly with Windows registry)
    6. `Settings...` (opens egui settings window)
    7. Separator
    8. `Exit`
  - **Left-Click / Double-Click**: Instantly opens the graphical Settings window.
  - **Explorer Restart Recovery**: Automatically detects `TaskbarCreated` and re-registers the tray icon if Windows Explorer restarts.
- **Modern Graphical Settings UI**:
  - Built with `egui` and `eframe` (Glow/OpenGL backend).
  - Displays real-time idle time and live time-to-shutdown countdown.
  - "Test Warning Popup (10s)" button to test popup visuals and dismissal without shutting down.
  - Runs in a lightweight on-demand child process (`idleoff.exe --settings`) so egui memory is only allocated while settings are open.
- **Robust Privilege Escalation**:
  - Acquires `SE_SHUTDOWN_NAME` via `AdjustTokenPrivileges` and issues planned shutdown reason codes via `ExitWindowsEx`.
  - Re-verifies user input immediately prior to shutdown.
- **Zero-Dependency Persistence**:
  - Persistent settings stored in `%APPDATA%\IdleOff\config.toml` with a tiny handwritten parser and atomic file writing (temp file + rename).
- **Single-Instance Protection**:
  - Guarded by a named kernel mutex (`Global\IdleOff_Daemon_Mutex_9F8D2E`) and window class detection (`FindWindowW`).
  - Double-clicking the executable while running opens/focuses the settings window or displays a friendly notification.

---

## 🏗️ Architecture

IdleOff implements a **Two-Mode Single Executable** architecture:

```
┌────────────────────────────────────────────────────────────┐
│                       idleoff.exe                          │
└─────────────┬────────────────────────────────┬─────────────┘
              │                                │
      (Default / No args)                (--settings)
              │                                │
              ▼                                ▼
┌───────────────────────────┐    IPC     ┌───────────────────────────┐
│     Background Daemon     │◄──────────►│    Egui Settings Window   │
│  - Pure Win32 Event Loop  │  (Window   │  - Realtime Telemetry     │
│  - System Tray Icon       │  Messages  │  - Radio Timeouts         │
│  - Idle Timer & Polling   │     +      │  - Test Popup Button      │
│  - Native Warning Dialog  │  Config)   │  - Auto-terminates on (X) │
│  - Privilege & Shutdown   │            │                           │
│  [ ~4 MB RAM | 0% CPU ]   │            │ [ Exits when closed ]     │
└───────────────────────────┘            └───────────────────────────┘
```

1. **Daemon Mode (`idleoff.exe`)**:
   - The primary resident process. Loads zero GUI frameworks.
   - Handles Win32 timers, tray menus, power broadcasts, and shutdown logic.
2. **Settings Mode (`idleoff.exe --settings`)**:
   - Launched as a child process when the user left-clicks the tray icon or chooses "Settings...".
   - Communicates changes to the daemon via registered window messages (`IdleOff_ConfigChanged`, `IdleOff_TestPopup`) and shared config file.
   - Fully terminates when closed, releasing all UI and graphics memory.

---

## 🚀 Getting Started

### Prerequisites
- Windows 10 or Windows 11 (x64)
- Rust toolchain (`stable-x86_64-pc-windows-msvc`)

### Build Release Binary
To compile the production release binary:

```powershell
cargo build --release
```

The resulting executable is located at:
```
target\release\idleoff.exe
```

### Run
Simply double-click `idleoff.exe` or execute from PowerShell:
```powershell
.\target\release\idleoff.exe
```

---

## 🛠️ CLI Options

| Flag | Description |
|---|---|
| *(none)* | Starts the background resident daemon with system tray icon. |
| `--settings` | Opens the graphical settings window. |
| `--dry-run` | Runs all logic and countdowns without performing an actual shutdown (displays an informational message box when shutdown triggers). |
| `--test-seconds` | Interprets all timeouts as seconds instead of minutes (5s, 15s, 30s, 60s, 120s) and reduces warning lead to 3s for rapid testing. |
| `--log` | Enables detailed timestamped diagnostic logging to `%APPDATA%\IdleOff\idleoff.log`. |
| `--silent` / `--quiet` | Suppresses any interactive message boxes if launched when an instance is already running. |

### Diagnostic Testing Command
To test the complete workflow in seconds without shutting down your PC:
```powershell
.\target\release\idleoff.exe --dry-run --test-seconds --log
```

---

## ⚙️ Configuration

Settings are saved in `%APPDATA%\IdleOff\config.toml`:

```toml
# IdleOff Configuration File
enabled = true
timeout_minutes = 30
show_popup = true
```

---

## 🧪 Running Unit Tests

Run the built-in test suite covering idle tick math, wrap-around handling, threshold logic, menu mappings, and config parsing:

```powershell
cargo test
```
