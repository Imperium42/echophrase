// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Error helpers for Tauri commands.
//!
//! Commands return `CmdResult<T>` (`Result<T, String>`); `ToCmdErr` converts any
//! `Display` error into that string form at the IPC boundary.

use std::path::PathBuf;

/// Shorthand return type for Tauri commands that serialize errors as plain strings.
///
/// Use `CmdResult<T>` in `#[tauri::command]` functions instead of `Result<T, String>`
/// to keep signatures concise. The Tauri IPC boundary requires `E: Serialize`, and
/// `String` satisfies that.
pub type CmdResult<T> = Result<T, String>;

/// Extension trait that converts any `Result<T, E: Display>` into `CmdResult<T>`.
///
/// # Example
/// ```ignore
/// use crate::error::ToCmdErr;
///
/// fn load() -> CmdResult<u32> {
///     some_fallible_op().to_cmd()
/// }
/// ```
pub trait ToCmdErr<T> {
    /// Map the error variant to `String` via `Display`.
    fn to_cmd(self) -> CmdResult<T>;
}

impl<T, E: std::fmt::Display> ToCmdErr<T> for Result<T, E> {
    fn to_cmd(self) -> CmdResult<T> {
        self.map_err(|e| e.to_string())
    }
}

/**
 * Helper to convert PathBuf to String for serialization
 */
#[allow(dead_code)]
pub fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}
