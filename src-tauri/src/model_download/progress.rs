// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/// Format seconds into a user-friendly elapsed time string.
pub fn format_time(seconds: u64) -> String {
    if seconds == 0 {
        return "".to_string();
    }

    let minutes = seconds / 60;
    let secs = seconds % 60;

    if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}
