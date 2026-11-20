// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Per-user filesystem layout for CLI state.
//!
//! Everything the CLI persists lives under a single base directory:
//!
//! ```text
//! <base>/
//!   run/          ephemeral coordination files
//!     session.json   the active recording (pid, output path, start time)
//!     stop.flag      created by `stop`, polled by the recorder
//!     worker.log     stdout/stderr of the detached background worker
//!     ipc-token      bearer token for the desktop app's local IPC server
//!     ipc-state.json why the IPC server is down (disabled/running)
//!   recordings/   default location for captured WAV files
//! ```
//!
//! The desktop app writes `run/ipc-token` under this exact same base
//! directory (see `src-tauri/src/ipc/paths.rs`, which mirrors the resolution
//! order below) so the CLI and the app agree on where to find it without
//! either side needing to know about the other's install layout.
//!
//! The base directory is, in order of precedence:
//!   1. `$ECHOPHRASE_HOME` if set,
//!   2. the platform data directory + `echophrase-cli`,
//!   3. the system temp directory as a last resort.

use std::path::PathBuf;

/// Root directory for all CLI state.
pub fn base_dir() -> PathBuf {
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

/// Directory for ephemeral coordination files.
pub fn run_dir() -> PathBuf {
    let mut d = base_dir();
    d.push("run");
    d
}

/// Default directory for captured recordings.
pub fn recordings_dir() -> PathBuf {
    let mut d = base_dir();
    d.push("recordings");
    d
}

/// Path to the active-session descriptor.
pub fn session_file() -> PathBuf {
    let mut d = run_dir();
    d.push("session.json");
    d
}

/// Path to the stop-request flag file.
pub fn stop_flag() -> PathBuf {
    let mut d = run_dir();
    d.push("stop.flag");
    d
}

/// Path to the background worker's log file.
pub fn worker_log() -> PathBuf {
    let mut d = run_dir();
    d.push("worker.log");
    d
}

/// Path to the desktop app's local IPC bearer token file.
///
/// Written by the running desktop app (see `src-tauri/src/ipc/paths.rs`) and
/// read by `echophrase transcribe` / the MCP `transcribe` tool to
/// authenticate against the app's localhost-only HTTP server.
pub fn ipc_token_file() -> PathBuf {
    let mut d = run_dir();
    d.push("ipc-token");
    d
}

/// Path to the desktop app's small JSON state file recording why the IPC
/// listener is not currently serving requests (`disabled`, or
/// `running`). Written by the running desktop app (see
/// `src-tauri/src/ipc/paths.rs::ipc_state_file`, which mirrors this
/// resolution order) and read by `echophrase transcribe` / the MCP
/// `transcribe` tool to turn a bare connection failure into an actionable
/// error. Missing, empty, or unparseable is treated the same as "app not
/// running".
pub fn ipc_state_file() -> PathBuf {
    let mut d = run_dir();
    d.push("ipc-state.json");
    d
}

/// Create the `run/` and `recordings/` directories if they do not exist.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(run_dir())?;
    std::fs::create_dir_all(recordings_dir())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// SAFETY: `set_var`/`remove_var` are unsafe in edition 2024 because env
    /// mutation is process-global; `#[serial]` guarantees no other thread races.
    fn set_home(value: Option<&str>) {
        unsafe {
            match value {
                Some(v) => std::env::set_var("ECHOPHRASE_HOME", v),
                None => std::env::remove_var("ECHOPHRASE_HOME"),
            }
        }
    }

    #[test]
    #[serial]
    fn echophrase_home_overrides_layout() {
        set_home(Some("/tmp/echo-home-test"));
        let base = base_dir();
        assert_eq!(base, PathBuf::from("/tmp/echo-home-test"));
        assert_eq!(run_dir(), base.join("run"));
        assert_eq!(recordings_dir(), base.join("recordings"));
        assert_eq!(session_file(), base.join("run").join("session.json"));
        assert_eq!(stop_flag(), base.join("run").join("stop.flag"));
        set_home(None);
    }

    #[test]
    #[serial]
    fn empty_or_unset_home_falls_back_to_platform_dir() {
        set_home(Some(""));
        assert!(base_dir().ends_with("echophrase-cli"));
        set_home(None);
        assert!(base_dir().ends_with("echophrase-cli"));
    }
}
