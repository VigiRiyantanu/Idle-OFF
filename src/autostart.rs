//! Windows autostart management using the current user's registry Run key.
//!
//! Manipulates `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` to enable or disable
//! launching IdleOff automatically when the user signs in.

use crate::logger;
use crate::util::to_wide;
use std::env;
use std::ptr::null_mut;
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
};

const RUN_KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "IdleOff";

/// Checks if IdleOff is configured to launch automatically on Windows login
/// by querying the actual registry value directly.
pub fn is_autostart_enabled() -> bool {
    let wide_key = to_wide(RUN_KEY_PATH);
    let wide_val = to_wide(VALUE_NAME);
    let mut hkey: HKEY = null_mut();

    // SAFETY: We pass a valid wide string and pointer to receive HKEY.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide_key.as_ptr(),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        )
    };

    if status != 0 {
        return false;
    }

    // SAFETY: Passing valid HKEY, value name, and checking if the value exists.
    let query_status = unsafe {
        RegQueryValueExW(
            hkey,
            wide_val.as_ptr(),
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
        )
    };

    // SAFETY: Closing open registry key.
    unsafe {
        RegCloseKey(hkey);
    }

    let enabled = query_status == 0;
    logger::log(&format!("Autostart query from registry: {enabled}"));
    enabled
}

/// Enables or disables autostart in the Windows registry for the current user.
pub fn set_autostart_enabled(enabled: bool) -> Result<(), &'static str> {
    let wide_key = to_wide(RUN_KEY_PATH);
    let wide_val = to_wide(VALUE_NAME);
    let mut hkey: HKEY = null_mut();

    // SAFETY: Opening the Run key with KEY_SET_VALUE permissions.
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide_key.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
    };

    if status != 0 {
        logger::log("Autostart error: Failed to open HKCU Run key for writing");
        return Err("Failed to open HKCU Run key for writing");
    }

    let result = if enabled {
        let exe_path = env::current_exe().map_err(|_| "Failed to get current executable path")?;
        // Format path as a quoted string to handle spaces in folder paths safely
        let val_str = format!("\"{}\"", exe_path.display());
        let val_wide = to_wide(&val_str);
        let byte_len = (val_wide.len() * std::mem::size_of::<u16>()) as u32;

        // SAFETY: Setting REG_SZ value with the null-terminated wide path.
        let set_status = unsafe {
            RegSetValueExW(
                hkey,
                wide_val.as_ptr(),
                0,
                REG_SZ,
                val_wide.as_ptr() as *const u8,
                byte_len,
            )
        };
        if set_status == 0 {
            logger::log(&format!("Autostart enabled in registry: {val_str}"));
            Ok(())
        } else {
            logger::log(&format!(
                "Autostart error: RegSetValueExW failed ({set_status})"
            ));
            Err("Failed to set registry value")
        }
    } else {
        // SAFETY: Deleting the value if present. It is acceptable if it does not exist.
        let del_status = unsafe { RegDeleteValueW(hkey, wide_val.as_ptr()) };
        // ERROR_FILE_NOT_FOUND (2) is considered success when disabling
        if del_status == 0 || del_status == 2 {
            logger::log("Autostart disabled in registry");
            Ok(())
        } else {
            logger::log(&format!(
                "Autostart error: RegDeleteValueW failed ({del_status})"
            ));
            Err("Failed to delete registry value")
        }
    };

    // SAFETY: Closing open registry key.
    unsafe {
        RegCloseKey(hkey);
    }

    result
}
