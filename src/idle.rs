//! User idle time measurement using Win32 `GetLastInputInfo`.

use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Computes the elapsed idle time in milliseconds from tick counters.
///
/// Handles the 32-bit tick counter wrap-around (which occurs every ~49.7 days of continuous
/// system uptime) using two's-complement modular arithmetic (`wrapping_sub`).
/// As long as the elapsed idle duration is less than 49.7 days, `wrapping_sub` yields
/// the mathematically exact difference.
#[inline]
pub fn compute_idle_ms(current_ticks: u32, last_input_ticks: u32) -> u32 {
    current_ticks.wrapping_sub(last_input_ticks)
}

/// Computes elapsed idle time in whole seconds.
#[inline]
pub fn compute_idle_seconds(current_ticks: u32, last_input_ticks: u32) -> u32 {
    compute_idle_ms(current_ticks, last_input_ticks) / 1000
}

/// Queries the operating system for the current system-wide user idle time in seconds.
///
/// This checks both mouse movement/clicks and keyboard inputs across all desktop sessions.
pub fn get_idle_seconds() -> Result<u32, &'static str> {
    let mut lpi = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    // SAFETY: `lpi` is a valid stack-allocated LASTINPUTINFO struct whose `cbSize`
    // member is correctly initialized to the struct's byte size prior to invocation.
    let success = unsafe { GetLastInputInfo(&mut lpi) };
    if success == 0 {
        return Err("GetLastInputInfo failed");
    }

    // SAFETY: `GetTickCount` has no safety prerequisites and cannot fail.
    let current_ticks = unsafe { GetTickCount() };

    Ok(compute_idle_seconds(current_ticks, lpi.dwTime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_idle_ms_normal() {
        let last = 100_000;
        let current = 125_000;
        assert_eq!(compute_idle_ms(current, last), 25_000);
        assert_eq!(compute_idle_seconds(current, last), 25);
    }

    #[test]
    fn test_compute_idle_ms_zero() {
        let last = 50_000;
        let current = 50_000;
        assert_eq!(compute_idle_ms(current, last), 0);
        assert_eq!(compute_idle_seconds(current, last), 0);
    }

    #[test]
    fn test_compute_idle_ms_wraparound() {
        // System uptime crosses 2^32 - 1 (49.7 days)
        // Last input was 5,000 ms before overflow: u32::MAX - 4999
        // Current tick is 5,000 ms after overflow: 5000
        let last = u32::MAX - 4999;
        let current = 5000;
        let elapsed = compute_idle_ms(current, last);
        assert_eq!(elapsed, 10_000);
        assert_eq!(compute_idle_seconds(current, last), 10);
    }
}
