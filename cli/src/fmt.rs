// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Small human-readable formatting helpers for CLI output.

/// Format a duration in seconds as e.g. `7s` or `1m 5s`.
pub fn duration(secs: u64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Format a byte count as B / KiB / MiB.
pub fn bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    if n >= MB {
        format!("{:.1} MiB", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{:.1} KiB", n as f64 / KB as f64)
    } else {
        format!("{n} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_formats_sub_minute() {
        assert_eq!(duration(0), "0s");
        assert_eq!(duration(7), "7s");
        assert_eq!(duration(59), "59s");
    }

    #[test]
    fn duration_formats_minutes() {
        assert_eq!(duration(65), "1m 5s");
        assert_eq!(duration(600), "10m 0s");
    }

    #[test]
    fn bytes_formats_each_unit() {
        assert_eq!(bytes(256), "256 B");
        assert_eq!(bytes(1024), "1.0 KiB");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(1024 * 1024), "1.0 MiB");
    }
}
