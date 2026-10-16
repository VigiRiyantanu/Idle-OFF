//! Configuration storage and handwritten lightweight TOML parser.
//!
//! Stores user preferences in `%APPDATA%\IdleOff\config.toml` without external
//! dependencies like `serde` or `toml`, ensuring a minimal binary footprint.
//! Writes are performed atomically (writing to a temporary file then renaming).

use crate::logger;
use std::fs;
use std::io;
use std::path::PathBuf;

/// Application settings persisted to disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Whether idle detection and automatic shutdown are enabled.
    pub enabled: bool,
    /// Configured idle timeout in minutes (allowed: 5, 15, 30, 60, 120).
    pub timeout_minutes: u32,
    /// Whether to display a 3-minute countdown popup before shutdown.
    pub show_popup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_minutes: 30,
            show_popup: true,
        }
    }
}

impl Config {
    /// Resolves the default configuration file path `%APPDATA%\IdleOff\config.toml`.
    pub fn config_path() -> Option<PathBuf> {
        let appdata = std::env::var_os("APPDATA")?;
        let mut path = PathBuf::from(appdata);
        path.push("IdleOff");
        path.push("config.toml");
        Some(path)
    }

    /// Loads configuration from disk, falling back to defaults if the file is missing or invalid.
    pub fn load() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => {
                logger::log("Config load failed: APPDATA path not found. Using defaults.");
                return Self::default();
            }
        };

        match fs::read_to_string(&path) {
            Ok(content) => {
                let cfg = parse_config_str(&content);
                logger::log(&format!(
                    "Config loaded: enabled={}, timeout={}m, popup={}",
                    cfg.enabled, cfg.timeout_minutes, cfg.show_popup
                ));
                cfg
            }
            Err(e) => {
                logger::log(&format!(
                    "Config file unreadable ({e}). Using default configuration."
                ));
                Self::default()
            }
        }
    }

    /// Saves configuration to disk atomically using a temporary file and rename.
    pub fn save(&self) -> io::Result<()> {
        let path = match Self::config_path() {
            Some(p) => p,
            None => {
                let err = io::Error::new(
                    io::ErrorKind::NotFound,
                    "APPDATA environment variable not found",
                );
                logger::log("Config save error: APPDATA environment variable not found");
                return Err(err);
            }
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = self.to_toml_string();

        // Atomic write pattern: write to a temporary file in the same directory, then rename
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &content)?;

        // On Windows, if destination exists, remove it first or replace it
        if path.exists() {
            let _ = fs::remove_file(&path);
        }

        if let Err(e) = fs::rename(&temp_path, &path) {
            // Fallback: write directly if rename failed
            let _ = fs::write(&path, content);
            let _ = fs::remove_file(&temp_path);
            logger::log(&format!("Config rename fallback used: {e}"));
        } else {
            logger::log("Config saved atomically to disk.");
        }

        Ok(())
    }

    /// Serializes configuration to a clean TOML string representation.
    pub fn to_toml_string(&self) -> String {
        format!(
            "# IdleOff Configuration File\n\
             enabled = {}\n\
             timeout_minutes = {}\n\
             show_popup = {}\n",
            self.enabled, self.timeout_minutes, self.show_popup
        )
    }
}

/// Parses a simple key-value TOML string into a [`Config`].
///
/// Unrecognized lines, comments (starting with `#`), and malformed keys are safely ignored,
/// preserving sensible defaults for any missing or invalid fields.
pub fn parse_config_str(content: &str) -> Config {
    let mut config = Config::default();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, val)) = trimmed.split_once('=') {
            let key = key.trim();
            let val = val.trim();

            match key {
                "enabled" => {
                    if let Ok(b) = val.parse::<bool>() {
                        config.enabled = b;
                    }
                }
                "timeout_minutes" => {
                    if let Ok(num) = val.parse::<u32>() {
                        // Validate timeout is one of the 5 supported options
                        if matches!(num, 5 | 15 | 30 | 60 | 120) {
                            config.timeout_minutes = num;
                        }
                    }
                }
                "show_popup" => {
                    if let Ok(b) = val.parse::<bool>() {
                        config.show_popup = b;
                    }
                }
                _ => {}
            }
        }
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.timeout_minutes, 30);
        assert!(cfg.show_popup);
    }

    #[test]
    fn test_parse_valid_toml() {
        let toml = r#"
            # Settings
            enabled = false
            timeout_minutes = 60
            show_popup = false
        "#;
        let cfg = parse_config_str(toml);
        assert!(!cfg.enabled);
        assert_eq!(cfg.timeout_minutes, 60);
        assert!(!cfg.show_popup);
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml = "timeout_minutes = 15";
        let cfg = parse_config_str(toml);
        assert!(cfg.enabled); // default
        assert_eq!(cfg.timeout_minutes, 15);
        assert!(cfg.show_popup); // default
    }

    #[test]
    fn test_parse_corrupt_toml() {
        let toml = "random junk = ???\n!!!invalid syntax";
        let cfg = parse_config_str(toml);
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn test_parse_invalid_timeout_fallback() {
        let toml = "timeout_minutes = 42"; // 42 is not a supported timeout option
        let cfg = parse_config_str(toml);
        assert_eq!(cfg.timeout_minutes, 30); // preserved default
    }

    #[test]
    fn test_round_trip() {
        let original = Config {
            enabled: false,
            timeout_minutes: 120,
            show_popup: false,
        };
        let serialized = original.to_toml_string();
        let parsed = parse_config_str(&serialized);
        assert_eq!(original, parsed);
    }
}
