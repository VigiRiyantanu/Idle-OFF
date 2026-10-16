//! Lightweight file logger active only when `--log` command-line flag is passed.
//!
//! Logs events to `%APPDATA%\IdleOff\idleoff.log` without external dependencies.
//! All file operations are non-panicking and fail-safe.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

static LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enables or disables file logging.
pub fn set_logging_enabled(enabled: bool) {
    LOGGING_ENABLED.store(enabled, Ordering::Relaxed);
}

/// Returns whether file logging is enabled.
pub fn is_logging_enabled() -> bool {
    LOGGING_ENABLED.load(Ordering::Relaxed)
}

/// Resolves the log file path: `%APPDATA%\IdleOff\idleoff.log`.
fn log_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let mut path = PathBuf::from(appdata);
    path.push("IdleOff");
    path.push("idleoff.log");
    Some(path)
}

/// Appends a message line with a timestamp to the log file if logging is enabled.
pub fn log(msg: &str) {
    if !is_logging_enabled() {
        return;
    }

    let Some(path) = log_path() else { return };

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let now = std::time::SystemTime::now();
        let duration = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();
        let _ = writeln!(file, "[{secs}.{millis:03}] {msg}");
    }
}
