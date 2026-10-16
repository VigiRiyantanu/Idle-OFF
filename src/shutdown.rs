//! Windows privilege escalation and clean system shutdown execution.

use crate::idle::get_idle_seconds;
use crate::logger;
use crate::util::to_wide;
use std::fmt;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, FALSE, HANDLE, LUID};
use windows_sys::Win32::Security::{
    AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED,
    SE_SHUTDOWN_NAME, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows_sys::Win32::System::Shutdown::{
    ExitWindowsEx, EWX_POWEROFF, EWX_SHUTDOWN, SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER,
    SHTDN_REASON_MINOR_OTHER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

/// Error types that can occur during system shutdown sequence.
#[derive(Debug)]
pub enum ShutdownError {
    OpenProcessTokenFailed(u32),
    LookupPrivilegeFailed(u32),
    AdjustTokenPrivilegesFailed(u32),
    AbortedByUserActivity,
    ExitWindowsExFailed(u32),
}

impl fmt::Display for ShutdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenProcessTokenFailed(code) => {
                write!(f, "Failed to open process token (err: {code})")
            }
            Self::LookupPrivilegeFailed(code) => {
                write!(
                    f,
                    "Failed to lookup SE_SHUTDOWN_NAME privilege (err: {code})"
                )
            }
            Self::AdjustTokenPrivilegesFailed(code) => {
                write!(f, "Failed to adjust token privileges (err: {code})")
            }
            Self::AbortedByUserActivity => {
                write!(
                    f,
                    "Shutdown aborted: user input detected immediately before shutdown"
                )
            }
            Self::ExitWindowsExFailed(code) => {
                write!(f, "ExitWindowsEx failed to execute shutdown (err: {code})")
            }
        }
    }
}

impl std::error::Error for ShutdownError {}

/// Acquires the `SE_SHUTDOWN_NAME` privilege for the current process.
fn acquire_shutdown_privilege() -> Result<(), ShutdownError> {
    let mut token: HANDLE = null_mut();

    // SAFETY: Querying the current process token with TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY.
    let ok = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
    };
    if ok == FALSE {
        let err = unsafe { GetLastError() };
        logger::log(&format!("Privilege error: OpenProcessToken failed ({err})"));
        return Err(ShutdownError::OpenProcessTokenFailed(err));
    }

    let mut luid = LUID {
        LowPart: 0,
        HighPart: 0,
    };

    // SAFETY: Looking up LUID for SE_SHUTDOWN_NAME (SeShutdownPrivilege).
    let ok = unsafe { LookupPrivilegeValueW(null_mut(), SE_SHUTDOWN_NAME, &mut luid) };
    if ok == FALSE {
        let err = unsafe { GetLastError() };
        unsafe { CloseHandle(token) };
        logger::log(&format!(
            "Privilege error: LookupPrivilegeValueW failed ({err})"
        ));
        return Err(ShutdownError::LookupPrivilegeFailed(err));
    }

    let tp = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };

    // SAFETY: Adjusting token privilege to enable SE_SHUTDOWN_NAME.
    let ok = unsafe {
        AdjustTokenPrivileges(
            token,
            FALSE,
            &tp,
            std::mem::size_of::<TOKEN_PRIVILEGES>() as u32,
            null_mut(),
            null_mut(),
        )
    };
    let err = unsafe { GetLastError() };
    unsafe { CloseHandle(token) };

    if ok == FALSE || err != 0 {
        logger::log(&format!(
            "Privilege error: AdjustTokenPrivileges failed ({err})"
        ));
        return Err(ShutdownError::AdjustTokenPrivilegesFailed(err));
    }

    logger::log("Privilege acquired: SE_SHUTDOWN_NAME enabled successfully");
    Ok(())
}

/// Executes the system shutdown, optionally running in dry-run mode.
///
/// Immediately prior to issuing the shutdown call, user idle time is re-queried.
/// If user activity (keyboard/mouse) was detected within the last 2 seconds,
/// the shutdown call is aborted to protect against accidental shutdown.
pub fn execute_shutdown(dry_run: bool) -> Result<(), ShutdownError> {
    logger::log(&format!("execute_shutdown called (dry_run={dry_run})"));

    // Safety check: Never shut down if user provided recent input
    if let Ok(idle_secs) = get_idle_seconds() {
        if idle_secs < 2 {
            logger::log(&format!(
                "Shutdown cancelled: user input detected (idle={idle_secs}s)"
            ));
            return Err(ShutdownError::AbortedByUserActivity);
        }
    }

    if dry_run {
        logger::log("DRY RUN: Showing notification dialog instead of shutting down");
        let title = to_wide("IdleOff - Dry Run");
        let msg = to_wide("DRY RUN: Idle threshold reached!\nShutdown would execute now.\n(System shutdown was not triggered).");
        // SAFETY: Showing an informational modal dialog in dry-run mode.
        unsafe {
            MessageBoxW(
                null_mut(),
                msg.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        return Ok(());
    }

    acquire_shutdown_privilege()?;

    let flags = EWX_SHUTDOWN | EWX_POWEROFF;
    let reason = SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_MINOR_OTHER | SHTDN_REASON_FLAG_PLANNED;

    // SAFETY: Invoking ExitWindowsEx with standard shutdown and poweroff flags and planned reason code.
    let ok = unsafe { ExitWindowsEx(flags, reason) };
    if ok == FALSE {
        let err = unsafe { GetLastError() };
        logger::log(&format!(
            "Shutdown execution failed: ExitWindowsEx returned error {err}"
        ));
        return Err(ShutdownError::ExitWindowsExFailed(err));
    }

    logger::log("Shutdown execution issued successfully via ExitWindowsEx");
    Ok(())
}
