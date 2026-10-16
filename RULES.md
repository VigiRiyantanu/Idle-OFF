# IdleOff Development Rules & Engineering Standards

These rules are strictly enforced for all code contributions to the **IdleOff** codebase.

---

## 1. Zero-Warning & Quality Policy

1. **Clippy Enforcement**: The codebase must compile with **zero warnings** under `cargo clippy -- -D warnings`.
2. **Formatting**: All source code must be formatted using `cargo fmt` without exception.
3. **No Dead Code**: Unused imports, unused structs, or obsolete constants must be removed.
4. **Documentation**: Every public function, struct, and enum variant must have a concise `///` doc comment explaining its purpose and behavior.

---

## 2. Safety & `unsafe` Discipline

1. **Mandatory Safety Comments**: Every `unsafe` block must be preceded by a `// SAFETY:` comment explicitly detailing why the invocation is sound (e.g., initialized structs, valid pointers, lifetime safety).
2. **Handle Cleanup**: Every allocated Win32 resource (`HKEY`, `HICON`, `HMENU`, `HANDLE`, `HWND`) must be explicitly released using its corresponding API (`RegCloseKey`, `DestroyMenu`, `DestroyWindow`, `CloseHandle`) or via standard `Drop` implementations.
3. **Pointers & Null Checks**: Check all handles against null pointers before use. Do not assume Win32 API calls always succeed.

---

## 3. Concurrency & Reentrancy Rules

1. **No Reentrant RefCell Borrows**:
   - `RefCell::borrow_mut()` must never be held across calls that can invoke nested message loops (such as `TrackPopupMenu` or `MessageBoxW`).
   - Use `TrackPopupMenuEx` with `TPM_RETURNCMD` so menus return their command ID directly rather than routing through reentrant `WM_COMMAND`.
2. **Single-Threaded Daemon**:
   - The daemon process must remain strictly single-threaded, driven by the native Win32 message loop (`GetMessageW`).
   - Do not spawn background polling threads when a Win32 timer (`SetTimer`) on the message loop is available.
3. **No Heavy Runtimes in Daemon**:
   - Do not introduce Tokio, async runtimes, or network stacks into the background process.

---

## 4. UI & User Experience Rules

1. **Non-Activating Warning Window**:
   - The countdown warning dialog must use `WS_EX_TOPMOST | WS_EX_TOOLWINDOW` and be displayed with `SW_SHOWNOACTIVATE`.
   - It must never steal keyboard focus or interrupt the user while typing.
2. **Automatic Dismissal on Input**:
   - If the user moves the mouse or types on the keyboard during the warning countdown, the warning dialog must close automatically and reset the idle cycle.
3. **Pre-Shutdown Verification**:
   - Immediately before issuing a shutdown call, re-query `GetLastInputInfo`. If idle seconds < 2, abort shutdown immediately to prevent accidents.

---

## 5. Persistence & Configuration Rules

1. **Zero External Crates for Persistence**:
   - Do not pull in `serde` or `toml` crates for config storage. Use the lightweight handwritten parser in `src/config.rs`.
2. **Atomic Writes**:
   - Always write configuration to a temporary file in `%APPDATA%\IdleOff` and rename it to `config.toml` to prevent file corruption during sudden system shutdowns.
3. **Graceful Fallbacks**:
   - If `config.toml` is absent or corrupted, the application must silently fall back to sensible defaults (`enabled = true`, `timeout_minutes = 30`, `show_popup = true`).
