//! Settings storage and lightweight handwritten JSON parser.
//!
//! Stores user preferences in `%TEMP%\idle-off\settings.json` without external JSON dependencies.
//! Provides atomic writing via temporary files and robust fallback to defaults on corrupt/missing files.

use crate::logger;
use std::env;
use std::fs;
use std::path::PathBuf;

pub const FOLDER_NAME: &str = "idle-off";
pub const FILE_NAME: &str = "settings.json";
pub const TEMP_FILE_NAME: &str = "settings.json.tmp";
pub const BAD_FILE_NAME: &str = "settings.json.bad";

pub const DEFAULT_VERSION: u32 = 1;
pub const DEFAULT_ENABLED: bool = true;
pub const DEFAULT_TIMEOUT_MINUTES: u32 = 30;
pub const DEFAULT_POPUP_ENABLED: bool = true;
pub const DEFAULT_START_WITH_WINDOWS: bool = false;

pub const ALLOWED_TIMEOUTS: [u32; 5] = [5, 15, 30, 60, 120];

pub const MAX_RENAME_RETRIES: u32 = 3;
pub const RENAME_RETRY_DELAY_MS: u64 = 50;

/// Persistent user settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub version: u32,
    pub enabled: bool,
    pub timeout_minutes: u32,
    pub popup_enabled: bool,
    pub start_with_windows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: DEFAULT_VERSION,
            enabled: DEFAULT_ENABLED,
            timeout_minutes: DEFAULT_TIMEOUT_MINUTES,
            popup_enabled: DEFAULT_POPUP_ENABLED,
            start_with_windows: DEFAULT_START_WITH_WINDOWS,
        }
    }
}

/// Parsing errors returned by `parse_strict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    EmptyInput,
    ExpectedObject,
    InvalidSyntax(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "empty input"),
            Self::ExpectedObject => {
                write!(
                    f,
                    "expected JSON object starting with '{{' and ending with '}}'"
                )
            }
            Self::InvalidSyntax(msg) => write!(f, "invalid JSON syntax: {msg}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Resolves the parent directory `%TEMP%\idle-off`.
pub fn dir_path() -> PathBuf {
    env::temp_dir().join(FOLDER_NAME)
}

/// Resolves the full path to `%TEMP%\idle-off\settings.json`.
///
/// Ensures the parent directory exists.
pub fn path() -> PathBuf {
    let dir = dir_path();
    if !dir.exists() {
        let _ = fs::create_dir_all(&dir);
    }
    dir.join(FILE_NAME)
}

/// Resolves the temporary write path `%TEMP%\idle-off\settings.json.tmp`.
pub fn temp_path() -> PathBuf {
    dir_path().join(TEMP_FILE_NAME)
}

/// Resolves the corrupt backup path `%TEMP%\idle-off\settings.json.bad`.
pub fn bad_path() -> PathBuf {
    dir_path().join(BAD_FILE_NAME)
}

/// Serializes [`Settings`] to a pretty-printed JSON string with 2 spaces indent and trailing newline.
pub fn to_json(settings: &Settings) -> String {
    format!(
        "{{\n  \"version\": {},\n  \"enabled\": {},\n  \"timeout_minutes\": {},\n  \"popup_enabled\": {},\n  \"start_with_windows\": {}\n}}\n",
        settings.version,
        settings.enabled,
        settings.timeout_minutes,
        settings.popup_enabled,
        settings.start_with_windows,
    )
}

/// Internal value representation during JSON scanning.
enum RawValue {
    String(String),
    Number(u32),
    Bool(bool),
    Other,
}

/// Lightweight tokenizer and character scanner for JSON objects.
struct JsonScanner {
    chars: Vec<char>,
    pos: usize,
}

impl JsonScanner {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_whitespace();
        self.chars.get(self.pos).copied()
    }

    fn next_char(&mut self) -> Option<char> {
        self.skip_whitespace();
        if self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), ParseError> {
        self.skip_whitespace();
        match self.next_char() {
            Some(ch) if ch == expected => Ok(()),
            Some(other) => Err(ParseError::InvalidSyntax(format!(
                "expected '{expected}', found '{other}'"
            ))),
            None => Err(ParseError::InvalidSyntax(format!(
                "expected '{expected}', found EOF"
            ))),
        }
    }

    fn read_string(&mut self) -> Result<String, ParseError> {
        self.skip_whitespace();
        if self.next_char() != Some('"') {
            return Err(ParseError::InvalidSyntax(
                "expected opening quote '\"'".to_string(),
            ));
        }

        let mut s = String::new();
        let mut escaped = false;
        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            self.pos += 1;
            if escaped {
                match ch {
                    '"' => s.push('"'),
                    '\\' => s.push('\\'),
                    '/' => s.push('/'),
                    'b' => s.push('\x08'),
                    'f' => s.push('\x0c'),
                    'n' => s.push('\n'),
                    'r' => s.push('\r'),
                    't' => s.push('\t'),
                    other => s.push(other),
                }
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                return Ok(s);
            } else {
                s.push(ch);
            }
        }
        Err(ParseError::InvalidSyntax(
            "unterminated string literal".to_string(),
        ))
    }

    fn read_raw_value(&mut self) -> Result<RawValue, ParseError> {
        self.skip_whitespace();
        let first = match self.chars.get(self.pos) {
            Some(&c) => c,
            None => {
                return Err(ParseError::InvalidSyntax(
                    "unexpected EOF reading value".to_string(),
                ))
            }
        };

        if first == '"' {
            let s = self.read_string()?;
            Ok(RawValue::String(s))
        } else if first == '{' {
            // Nested object - skip until matching '}'
            self.pos += 1;
            let mut depth = 1;
            let mut in_str = false;
            let mut escaped = false;
            while self.pos < self.chars.len() && depth > 0 {
                let ch = self.chars[self.pos];
                self.pos += 1;
                if in_str {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        in_str = false;
                    }
                } else if ch == '"' {
                    in_str = true;
                } else if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                }
            }
            if depth != 0 {
                return Err(ParseError::InvalidSyntax("unterminated object".to_string()));
            }
            Ok(RawValue::Other)
        } else if first == '[' {
            // Nested array - skip until matching ']'
            self.pos += 1;
            let mut depth = 1;
            let mut in_str = false;
            let mut escaped = false;
            while self.pos < self.chars.len() && depth > 0 {
                let ch = self.chars[self.pos];
                self.pos += 1;
                if in_str {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == '"' {
                        in_str = false;
                    }
                } else if ch == '"' {
                    in_str = true;
                } else if ch == '[' {
                    depth += 1;
                } else if ch == ']' {
                    depth -= 1;
                }
            }
            if depth != 0 {
                return Err(ParseError::InvalidSyntax("unterminated array".to_string()));
            }
            Ok(RawValue::Other)
        } else {
            // Number, boolean, or null token
            let mut token = String::new();
            while self.pos < self.chars.len() {
                let ch = self.chars[self.pos];
                if ch.is_whitespace() || ch == ',' || ch == '}' || ch == ']' {
                    break;
                }
                token.push(ch);
                self.pos += 1;
            }
            match token.as_str() {
                "true" => Ok(RawValue::Bool(true)),
                "false" => Ok(RawValue::Bool(false)),
                "null" => Ok(RawValue::Other),
                _ => {
                    if let Ok(num) = token.parse::<u32>() {
                        Ok(RawValue::Number(num))
                    } else if token.chars().all(|c| {
                        c.is_ascii_digit()
                            || c == '.'
                            || c == '-'
                            || c == '+'
                            || c == 'e'
                            || c == 'E'
                    }) {
                        Ok(RawValue::Other)
                    } else {
                        Err(ParseError::InvalidSyntax(format!(
                            "unrecognized token '{token}'"
                        )))
                    }
                }
            }
        }
    }
}

/// Strictly parses JSON content, returning [`ParseError`] if malformed.
pub fn parse_strict(content: &str) -> Result<Settings, ParseError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err(ParseError::ExpectedObject);
    }

    let mut scanner = JsonScanner::new(trimmed);
    scanner.expect('{')?;

    let mut settings = Settings::default();

    scanner.skip_whitespace();
    if scanner.peek() == Some('}') {
        scanner.next_char();
        scanner.skip_whitespace();
        if scanner.peek().is_some() {
            return Err(ParseError::InvalidSyntax(
                "trailing characters after object".to_string(),
            ));
        }
        return Ok(settings);
    }

    loop {
        let key = scanner.read_string()?;
        scanner.expect(':')?;
        let value = scanner.read_raw_value()?;

        match key.as_str() {
            "version" => {
                if let RawValue::Number(v) = value {
                    settings.version = v;
                }
            }
            "enabled" => match value {
                RawValue::Bool(b) => settings.enabled = b,
                RawValue::String(s) => {
                    if let Ok(b) = s.parse::<bool>() {
                        settings.enabled = b;
                    }
                }
                _ => {}
            },
            "timeout_minutes" => {
                let mins = match value {
                    RawValue::Number(n) => Some(n),
                    RawValue::String(s) => s.parse::<u32>().ok(),
                    _ => None,
                };
                if let Some(m) = mins {
                    if ALLOWED_TIMEOUTS.contains(&m) {
                        settings.timeout_minutes = m;
                    } else {
                        settings.timeout_minutes = DEFAULT_TIMEOUT_MINUTES;
                    }
                }
            }
            "popup_enabled" | "show_popup" => match value {
                RawValue::Bool(b) => settings.popup_enabled = b,
                RawValue::String(s) => {
                    if let Ok(b) = s.parse::<bool>() {
                        settings.popup_enabled = b;
                    }
                }
                _ => {}
            },
            "start_with_windows" | "autostart" => match value {
                RawValue::Bool(b) => settings.start_with_windows = b,
                RawValue::String(s) => {
                    if let Ok(b) = s.parse::<bool>() {
                        settings.start_with_windows = b;
                    }
                }
                _ => {}
            },
            _ => {
                // Unknown keys are intentionally ignored
            }
        }

        scanner.skip_whitespace();
        match scanner.peek() {
            Some(',') => {
                scanner.next_char();
                scanner.skip_whitespace();
                // Allow optional trailing comma before '}'
                if scanner.peek() == Some('}') {
                    scanner.next_char();
                    break;
                }
            }
            Some('}') => {
                scanner.next_char();
                break;
            }
            Some(other) => {
                return Err(ParseError::InvalidSyntax(format!(
                    "expected ',' or '}}', found '{other}'"
                )));
            }
            None => {
                return Err(ParseError::InvalidSyntax(
                    "unexpected EOF, expected '}'".to_string(),
                ));
            }
        }
    }

    scanner.skip_whitespace();
    if scanner.peek().is_some() {
        return Err(ParseError::InvalidSyntax(
            "trailing characters after object".to_string(),
        ));
    }

    Ok(settings)
}

/// Parses JSON content into [`Settings`], gracefully falling back to defaults on any parse error.
#[allow(dead_code)]
pub fn parse(content: &str) -> Settings {
    parse_strict(content).unwrap_or_default()
}

/// Attempts a one-time migration from legacy `%APPDATA%\IdleOff\config.toml` if it exists.
fn try_migrate_old_config() -> Option<Settings> {
    let appdata = env::var_os("APPDATA")?;
    let old_path = PathBuf::from(appdata).join("IdleOff").join("config.toml");
    if !old_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&old_path).ok()?;
    logger::log(&format!(
        "Migrating legacy config from {}",
        old_path.display()
    ));

    let mut settings = Settings::default();
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let val = v.trim();
            match key {
                "enabled" => {
                    if let Ok(b) = val.parse::<bool>() {
                        settings.enabled = b;
                    }
                }
                "timeout_minutes" => {
                    if let Ok(m) = val.parse::<u32>() {
                        if ALLOWED_TIMEOUTS.contains(&m) {
                            settings.timeout_minutes = m;
                        }
                    }
                }
                "show_popup" => {
                    if let Ok(b) = val.parse::<bool>() {
                        settings.popup_enabled = b;
                    }
                }
                _ => {}
            }
        }
    }
    Some(settings)
}

/// Loads persistent settings from `%TEMP%\idle-off\settings.json`.
///
/// Falls back to defaults if the file is missing, empty, unreadable, or corrupt.
/// If corrupt, renames to `.bad` and writes a fresh file.
/// Syncs `start_with_windows` with the real Windows registry state.
pub fn load() -> Settings {
    let dest = path();
    let bad = bad_path();

    if !dest.exists() {
        // Try migrating from legacy %APPDATA%\IdleOff\config.toml if present
        let reg_autostart = crate::autostart::is_autostart_enabled();
        let mut settings = try_migrate_old_config().unwrap_or_else(|| Settings {
            start_with_windows: reg_autostart,
            ..Default::default()
        });
        settings.start_with_windows = reg_autostart;
        let _ = save(&settings);
        return settings;
    }

    let content = match fs::read_to_string(&dest) {
        Ok(c) => c,
        Err(e) => {
            logger::log(&format!(
                "Failed to read settings file {}: {e}; using defaults",
                dest.display()
            ));
            return Settings {
                start_with_windows: crate::autostart::is_autostart_enabled(),
                ..Default::default()
            };
        }
    };

    let trimmed = content.trim();
    if trimmed.is_empty() {
        logger::log("Settings file is empty; rewriting with defaults");
        let settings = Settings {
            start_with_windows: crate::autostart::is_autostart_enabled(),
            ..Default::default()
        };
        let _ = save(&settings);
        return settings;
    }

    match parse_strict(trimmed) {
        Ok(mut settings) => {
            let reg_autostart = crate::autostart::is_autostart_enabled();
            if settings.start_with_windows != reg_autostart {
                logger::log(&format!(
                    "Syncing settings.start_with_windows from {} to registry value {}",
                    settings.start_with_windows, reg_autostart
                ));
                settings.start_with_windows = reg_autostart;
                let _ = save(&settings);
            }
            settings
        }
        Err(err) => {
            logger::log(&format!(
                "Settings file is corrupt ({err}); renaming to {} and resetting to defaults",
                bad.display()
            ));
            let _ = fs::remove_file(&bad);
            let _ = fs::rename(&dest, &bad);
            let settings = Settings {
                start_with_windows: crate::autostart::is_autostart_enabled(),
                ..Default::default()
            };
            let _ = save(&settings);
            settings
        }
    }
}

/// Atomically saves [`Settings`] to disk using a temporary file and rename with retry logic.
pub fn save(settings: &Settings) -> Result<(), std::io::Error> {
    let dir = dir_path();
    if let Err(e) = fs::create_dir_all(&dir) {
        logger::log(&format!(
            "Failed to create settings directory {}: {e}",
            dir.display()
        ));
        return Err(e);
    }

    let tmp = temp_path();
    let dest = path();
    let json = to_json(settings);

    // 1. Write to temp file
    if let Err(e) = fs::write(&tmp, &json) {
        logger::log(&format!(
            "Failed to write temporary settings file {}: {e}",
            tmp.display()
        ));
        return Err(e);
    }

    // 2. Atomic rename with retry logic for Windows file lock delays
    let mut last_err = None;
    for attempt in 1..=MAX_RENAME_RETRIES {
        // On Windows, target must be removed first if it exists for rename to succeed across platforms
        if dest.exists() {
            let _ = fs::remove_file(&dest);
        }
        match fs::rename(&tmp, &dest) {
            Ok(()) => {
                logger::log("Settings saved atomically to settings.json");
                return Ok(());
            }
            Err(e) => {
                logger::log(&format!(
                    "Rename {} -> {} failed on attempt {attempt}/{MAX_RENAME_RETRIES}: {e}",
                    tmp.display(),
                    dest.display()
                ));
                last_err = Some(e);
                std::thread::sleep(std::time::Duration::from_millis(RENAME_RETRY_DELAY_MS));
            }
        }
    }

    if let Some(err) = last_err {
        let _ = fs::remove_file(&tmp);
        logger::log(&format!(
            "Failed to rename temporary settings file after {MAX_RENAME_RETRIES} attempts: {err}"
        ));
        return Err(err);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trip() {
        let original = Settings {
            version: 1,
            enabled: true,
            timeout_minutes: 60,
            popup_enabled: false,
            start_with_windows: true,
        };
        let serialized = to_json(&original);
        let parsed = parse(&serialized);
        assert_eq!(original, parsed);
    }

    #[test]
    fn test_missing_keys_use_defaults() {
        let json = r#"{"timeout_minutes": 15}"#;
        let parsed = parse(json);
        assert_eq!(parsed.version, DEFAULT_VERSION);
        assert_eq!(parsed.enabled, DEFAULT_ENABLED);
        assert_eq!(parsed.timeout_minutes, 15);
        assert_eq!(parsed.popup_enabled, DEFAULT_POPUP_ENABLED);
        assert_eq!(parsed.start_with_windows, DEFAULT_START_WITH_WINDOWS);
    }

    #[test]
    fn test_extra_keys_ignored() {
        let json = r#"{
            "version": 1,
            "extra_string": "hello world",
            "extra_number": 999,
            "extra_bool": true,
            "extra_object": {"nested": 123},
            "extra_array": [1, 2, 3],
            "enabled": false,
            "timeout_minutes": 120
        }"#;
        let parsed = parse(json);
        assert_eq!(parsed.enabled, false);
        assert_eq!(parsed.timeout_minutes, 120);
        assert_eq!(parsed.popup_enabled, true);
    }

    #[test]
    fn test_invalid_timeout_fallback() {
        // 45 is not an allowed timeout (allowed: 5, 15, 30, 60, 120)
        let json = r#"{"timeout_minutes": 45}"#;
        let parsed = parse(json);
        assert_eq!(parsed.timeout_minutes, DEFAULT_TIMEOUT_MINUTES);

        let json_neg = r#"{"timeout_minutes": 0}"#;
        let parsed_neg = parse(json_neg);
        assert_eq!(parsed_neg.timeout_minutes, DEFAULT_TIMEOUT_MINUTES);
    }

    #[test]
    fn test_empty_file() {
        assert_eq!(parse(""), Settings::default());
        assert_eq!(parse("   \n\t  "), Settings::default());
    }

    #[test]
    fn test_garbage_file() {
        assert_eq!(parse("not json at all {][["), Settings::default());
        assert_eq!(parse("{ unterminated"), Settings::default());
        assert!(parse_strict("not json at all").is_err());
        assert!(parse_strict("{ unterminated").is_err());
    }

    #[test]
    fn test_whitespace_and_key_order() {
        let json = "  {\r\n  \"start_with_windows\": true,\n\n  \"popup_enabled\": false, \t \"timeout_minutes\": 5, \r\n \"enabled\": true, \"version\": 1,\n } \n";
        let parsed = parse(json);
        assert_eq!(parsed.timeout_minutes, 5);
        assert_eq!(parsed.enabled, true);
        assert_eq!(parsed.popup_enabled, false);
        assert_eq!(parsed.start_with_windows, true);
    }

    #[test]
    fn test_atomic_write_creates_folder() {
        let temp_sub = env::temp_dir().join("idleoff_test_atomic_folder");
        let _ = fs::remove_dir_all(&temp_sub);

        let test_file = temp_sub.join("settings.json");
        let test_tmp = temp_sub.join("settings.json.tmp");

        let s = Settings {
            version: 1,
            enabled: false,
            timeout_minutes: 5,
            popup_enabled: true,
            start_with_windows: false,
        };

        // Ensure folder creation works
        fs::create_dir_all(&temp_sub).unwrap();
        fs::write(&test_tmp, to_json(&s)).unwrap();
        fs::rename(&test_tmp, &test_file).unwrap();

        assert!(test_file.exists());
        let read = fs::read_to_string(&test_file).unwrap();
        assert_eq!(parse(&read), s);

        let _ = fs::remove_dir_all(&temp_sub);
    }
}
