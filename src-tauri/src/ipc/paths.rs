// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Path resolution for the IPC token file.
//!
//! Mirrors `cli/src/paths.rs` exactly (same precedence order, same
//! `echophrase-cli` directory name) so the desktop app and the CLI agree on
//! where the bearer token lives without either side depending on the other's
//! crate. Keep the two in sync if one changes.

use std::path::PathBuf;

/// Root directory shared with the CLI's per-user state.
fn base_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("ECHOPHRASE_HOME")
        && !home.is_empty()
    {
        return PathBuf::from(home);
    }
    let mut dir = platform_data_dir();
    dir.push("echophrase-cli");
    dir
}

#[cfg(target_os = "windows")]
fn platform_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

#[cfg(target_os = "macos")]
fn platform_data_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push("Library");
        p.push("Application Support");
        return p;
    }
    std::env::temp_dir()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn platform_data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME")
        && !xdg.is_empty()
    {
        return PathBuf::from(xdg);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".local");
        p.push("share");
        return p;
    }
    std::env::temp_dir()
}

/// Directory for ephemeral coordination files (shared with the CLI).
fn run_dir() -> PathBuf {
    let mut d = base_dir();
    d.push("run");
    d
}

/// Path to the bearer-token file the CLI reads to authenticate IPC requests.
pub fn ipc_token_file() -> PathBuf {
    let mut d = run_dir();
    d.push("ipc-token");
    d
}

/// Path to the small JSON state file recording why the IPC listener is not
/// currently serving requests (`disabled` or `running`). Read by
/// the CLI/MCP so a connection failure can be attributed to the right cause
/// instead of a single generic "app not running" message. Missing, empty, or
/// unparseable is treated the same as "app not running" by the reader.
pub fn ipc_state_file() -> PathBuf {
    let mut d = run_dir();
    d.push("ipc-state.json");
    d
}

/// Create the `run/` directory if it does not exist.
pub fn ensure_run_dir() -> std::io::Result<()> {
    std::fs::create_dir_all(run_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without `ECHOPHRASE_HOME` set, the token path lands under the same
    /// `echophrase-cli/run/` directory the CLI's `paths::ipc_token_file()`
    /// resolves to. We do not mutate `ECHOPHRASE_HOME` here (no serial_test
    /// dev-dependency in this crate); the `ECHOPHRASE_HOME` override path is
    /// exercised end-to-end by the CLI's integration tests instead, which
    /// spin up a fake IPC server and a real desktop-app-shaped token file
    /// under an isolated temp `ECHOPHRASE_HOME`.
    #[test]
    fn default_layout_matches_cli_directory_name() {
        let path = ipc_token_file();
        assert!(
            path.ends_with("echophrase-cli/run/ipc-token") || {
                // Windows path separators.
                path.to_string_lossy()
                    .replace('\\', "/")
                    .ends_with("echophrase-cli/run/ipc-token")
            }
        );
    }
}
