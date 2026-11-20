// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! The active-recording session and the stop-request signal.
//!
//! Coordination between `record`, the background `worker`, and `stop` is done
//! entirely through files (see [`crate::paths`]) rather than OS signals, so the
//! same mechanism works identically on Linux, macOS, and Windows.

use anyhow::{Context, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::paths;

/// Descriptor for the recording that is currently in progress.
#[derive(Serialize, Deserialize, Debug, Clone, JsonSchema)]
pub struct Session {
    /// Process id of the recorder (the background worker, or the foreground
    /// `record` process itself).
    pub pid: u32,
    /// Absolute path of the WAV file being written.
    pub output_path: String,
    /// Unix timestamp (seconds) when recording started.
    pub started_at: u64,
    /// True when recording in the current terminal (`record --foreground`).
    pub foreground: bool,
}

impl Session {
    /// Load the active session, or `None` if nothing is recording.
    pub fn load() -> Result<Option<Session>> {
        let p = paths::session_file();
        if !p.exists() {
            return Ok(None);
        }
        let data =
            std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        let session = serde_json::from_str(&data).context("parsing session.json")?;
        Ok(Some(session))
    }

    /// Persist this session as the active one.
    pub fn save(&self) -> Result<()> {
        paths::ensure_dirs()?;
        let p = paths::session_file();
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&p, data).with_context(|| format!("writing {}", p.display()))?;
        Ok(())
    }

    /// Remove the active-session descriptor, marking the system idle.
    pub fn clear() -> Result<()> {
        let p = paths::session_file();
        if p.exists() {
            std::fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
        }
        Ok(())
    }
}

/// Current Unix time in seconds (0 if the clock is before the epoch).
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Ask the active recorder to stop by creating the stop-flag file.
pub fn request_stop() -> Result<()> {
    paths::ensure_dirs()?;
    std::fs::write(paths::stop_flag(), b"stop").context("writing stop flag")?;
    Ok(())
}

/// Whether a stop has been requested.
pub fn stop_requested() -> bool {
    paths::stop_flag().exists()
}

/// Remove the stop-flag file (called once the recorder has acknowledged it).
pub fn clear_stop() -> Result<()> {
    let p = paths::stop_flag();
    if p.exists() {
        std::fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    /// Point ECHOPHRASE_HOME at a fresh temp dir so session/stop files are
    /// isolated. Returns the guard; dropping it removes the dir.
    ///
    /// SAFETY: env mutation is process-global; `#[serial]` prevents races.
    fn isolated_home() -> TempDir {
        let dir = TempDir::new().unwrap();
        unsafe { std::env::set_var("ECHOPHRASE_HOME", dir.path()) };
        dir
    }

    fn clear_home() {
        unsafe { std::env::remove_var("ECHOPHRASE_HOME") };
    }

    #[test]
    #[serial]
    fn save_load_clear_roundtrip() {
        let _home = isolated_home();
        assert!(Session::load().unwrap().is_none());

        let s = Session {
            pid: 4242,
            output_path: "/tmp/out.wav".into(),
            started_at: 1_700_000_000,
            foreground: true,
        };
        s.save().unwrap();

        let loaded = Session::load().unwrap().expect("session should load");
        assert_eq!(loaded.pid, 4242);
        assert_eq!(loaded.output_path, "/tmp/out.wav");
        assert_eq!(loaded.started_at, 1_700_000_000);
        assert!(loaded.foreground);

        Session::clear().unwrap();
        assert!(Session::load().unwrap().is_none());
        clear_home();
    }

    #[test]
    #[serial]
    fn stop_flag_lifecycle() {
        let _home = isolated_home();
        assert!(!stop_requested());

        request_stop().unwrap();
        assert!(stop_requested());

        clear_stop().unwrap();
        assert!(!stop_requested());
        // Clearing an already-absent flag is a no-op, not an error.
        clear_stop().unwrap();
        clear_home();
    }
}
