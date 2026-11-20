// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/// Check if running macOS 13 (Ventura) or later.
///
/// Metal's `MTLDevice.architecture` selector requires macOS 13+.
/// On older macOS, `Device::new_metal(0)` succeeds but crashes with an
/// uncatchable Objective-C exception during actual GPU computation.
#[cfg(target_os = "macos")]
pub fn is_macos_13_or_later() -> bool {
    use std::process::Command;
    if let Ok(output) = Command::new("sw_vers").arg("-productVersion").output() {
        if let Ok(version) = String::from_utf8(output.stdout) {
            let parts: Vec<&str> = version.trim().split('.').collect();
            if let Some(major) = parts.first().and_then(|s| s.parse::<u32>().ok()) {
                return major >= 13;
            }
        }
    }
    false
}

#[allow(dead_code)]
#[cfg(not(target_os = "macos"))]
pub fn is_macos_13_or_later() -> bool {
    false
}
