//! Shared utility functions for UTF-16 wide-string conversions and formatting.

/// Converts a UTF-8 string slice to a null-terminated UTF-16 vector.
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Copies a UTF-8 string slice into a fixed-size UTF-16 buffer with null termination.
///
/// If `src` is longer than `dest.len() - 1`, the content is truncated to ensure
/// the final element is always a null terminator (`0`).
pub fn copy_to_wide_buffer(src: &str, dest: &mut [u16]) {
    if dest.is_empty() {
        return;
    }
    let max_chars = dest.len() - 1;
    let mut written = 0;
    for (i, ch) in src.encode_utf16().take(max_chars).enumerate() {
        dest[i] = ch;
        written += 1;
    }
    dest[written] = 0;
}

/// Formats a remaining duration in seconds as "mm:ss".
pub fn format_countdown(seconds: u32) -> String {
    let minutes = seconds / 60;
    let secs = seconds % 60;
    format!("{:02}:{:02}", minutes, secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_wide_null_terminated() {
        let wide = to_wide("Test");
        assert_eq!(
            wide,
            vec![b'T' as u16, b'e' as u16, b's' as u16, b't' as u16, 0]
        );
    }

    #[test]
    fn test_copy_to_wide_buffer_normal() {
        let mut buf = [0u16; 8];
        copy_to_wide_buffer("Hi", &mut buf);
        assert_eq!(buf[0], b'H' as u16);
        assert_eq!(buf[1], b'i' as u16);
        assert_eq!(buf[2], 0);
    }

    #[test]
    fn test_copy_to_wide_buffer_truncate() {
        let mut buf = [0u16; 4];
        copy_to_wide_buffer("Hello World", &mut buf);
        // Only 3 characters fit + 1 null terminator
        assert_eq!(buf[0], b'H' as u16);
        assert_eq!(buf[1], b'e' as u16);
        assert_eq!(buf[2], b'l' as u16);
        assert_eq!(buf[3], 0);
    }

    #[test]
    fn test_format_countdown() {
        assert_eq!(format_countdown(0), "00:00");
        assert_eq!(format_countdown(59), "00:59");
        assert_eq!(format_countdown(60), "01:00");
        assert_eq!(format_countdown(179), "02:59");
        assert_eq!(format_countdown(3600), "60:00");
    }
}
