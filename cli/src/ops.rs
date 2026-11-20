// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Structured, printless core logic shared by the CLI commands and the MCP
//! server (`commands/mcp.rs`).
//!
//! Each `commands/*.rs` module keeps its existing stdout formatting and
//! behavior; it just delegates the actual work to a function here and prints
//! the returned struct. The MCP tools call the same functions and serialize
//! the structs as JSON tool results instead of printing them, so the two
//! front ends never drift.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::paths;
use crate::recorder::TARGET_RATE;
use crate::session::{self, Session};

/// Result of starting a background recording.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RecordStarted {
    /// Absolute path of the WAV file being written.
    pub output_path: String,
    /// Process id of the detached background worker.
    pub pid: u32,
}

/// Start (or restart, with `force`) a background recording.
///
/// This is the same logic `echophrase record` (non-foreground) runs; it is
/// factored out here so the MCP `record` tool can call it directly without
/// duplicating the session bookkeeping.
pub fn start_background_recording(
    output: Option<PathBuf>,
    device: Option<String>,
    force: bool,
) -> Result<RecordStarted> {
    paths::ensure_dirs()?;

    if let Some(existing) = Session::load()? {
        if force {
            // Best-effort teardown of a stale session before starting fresh.
            session::request_stop().ok();
            std::thread::sleep(Duration::from_millis(300));
            session::clear_stop().ok();
            Session::clear().ok();
        } else {
            bail!(
                "a recording is already in progress (pid {}, file {}).\n\
                 Run `echophrase stop` to finish it, or pass force to override.",
                existing.pid,
                existing.output_path
            );
        }
    }

    // A leftover flag from a previous run would stop us immediately.
    session::clear_stop().ok();

    let output = output.unwrap_or_else(default_output);
    spawn_worker(output, device)
}

/// Spawn the detached `worker` process and record its session.
fn spawn_worker(output: PathBuf, device: Option<String>) -> Result<RecordStarted> {
    let exe = std::env::current_exe().context("locating current executable")?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths::worker_log())
        .context("opening worker log")?;

    let mut cmd = Command::new(exe);
    cmd.arg("worker").arg("--output").arg(&output);
    if let Some(dev) = &device {
        cmd.arg("--device").arg(dev);
    }
    cmd.stdin(Stdio::null())
        .stdout(log.try_clone().context("cloning worker log handle")?)
        .stderr(log);
    detach(&mut cmd);

    let child = cmd.spawn().context("spawning recording worker")?;
    let pid = child.id();

    Session {
        pid,
        output_path: output.to_string_lossy().into_owned(),
        started_at: session::now_unix(),
        foreground: false,
    }
    .save()?;

    Ok(RecordStarted {
        output_path: output.to_string_lossy().into_owned(),
        pid,
    })
}

/// Default output path: a timestamped file in the recordings directory.
pub fn default_output() -> PathBuf {
    let mut p = paths::recordings_dir();
    p.push(format!("echophrase-{}.wav", session::now_unix()));
    p
}

#[cfg(unix)]
fn detach(cmd: &mut Command) {
    // New process group so the worker is not killed by Ctrl+C in the terminal.
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
}

#[cfg(not(any(unix, windows)))]
fn detach(_cmd: &mut Command) {}

/// Outcome of requesting a stop.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct StopResult {
    /// Path of the WAV file that was (or is still being) finalized.
    pub wav_path: String,
    /// Process id of the recorder that was stopped.
    pub pid: u32,
    /// True if the recording being stopped was a foreground session (the
    /// foreground process finalizes and prints its own summary; this call
    /// only delivers the stop request).
    pub foreground: bool,
    /// True if the background worker did not confirm within the timeout.
    /// `wav_path` may still be finalizing in this case.
    pub timed_out: bool,
    /// Recording duration in seconds, if known.
    pub duration_secs: Option<u64>,
    /// Final file size in bytes, if known.
    pub size_bytes: Option<u64>,
}

/// Request a stop and, for background recordings, wait up to `timeout_secs`
/// for the worker to finalize the file.
pub fn stop(timeout_secs: u64) -> Result<StopResult> {
    let Some(sess) = Session::load()? else {
        bail!("no active recording. Start one with `echophrase record`.");
    };

    session::request_stop()?;

    if sess.foreground {
        return Ok(StopResult {
            wav_path: sess.output_path,
            pid: sess.pid,
            foreground: true,
            timed_out: false,
            duration_secs: None,
            size_bytes: None,
        });
    }

    let deadline = session::now_unix() + timeout_secs.max(1);
    loop {
        if Session::load()?.is_none() {
            break;
        }
        if session::now_unix() >= deadline {
            return Ok(StopResult {
                wav_path: sess.output_path,
                pid: sess.pid,
                foreground: false,
                timed_out: true,
                duration_secs: None,
                size_bytes: None,
            });
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    let path = Path::new(&sess.output_path);
    let elapsed = session::now_unix().saturating_sub(sess.started_at);
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    Ok(StopResult {
        wav_path: sess.output_path,
        pid: sess.pid,
        foreground: false,
        timed_out: false,
        duration_secs: Some(elapsed),
        size_bytes: Some(size),
    })
}

/// Whether a recording is in progress, and its session details if so.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct StatusResult {
    /// True if a recording is currently in progress.
    pub recording: bool,
    /// The active session, if `recording` is true.
    pub session: Option<Session>,
}

/// Report whether a recording is currently in progress.
pub fn status() -> Result<StatusResult> {
    let session = Session::load()?;
    Ok(StatusResult {
        recording: session.is_some(),
        session,
    })
}

/// Result of transcoding a WAV file to the canonical 16 kHz mono format.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TranscodeResult {
    /// Path of the written 16 kHz mono WAV file.
    pub output_path: String,
    /// Sample rate of the written file (always [`TARGET_RATE`]).
    pub sample_rate: u32,
    /// Sample rate the source file was recorded at, if different.
    pub source_rate: u32,
    /// Duration of the output file in seconds.
    pub duration_secs: u64,
    /// Size of the output file in bytes.
    pub size_bytes: u64,
    /// True if peak normalization was applied.
    pub normalized: bool,
}

/// Convert an existing WAV file to 16 kHz mono, optionally peak-normalized.
pub fn transcode(
    input: &Path,
    output: Option<PathBuf>,
    normalize: bool,
) -> Result<TranscodeResult> {
    use echo_audio::{NORMALIZE_TARGET_PEAK, sample, wav};

    if !input.exists() {
        bail!("input file not found: {}", input.display());
    }

    let buffer = wav::read_wav(input)?;
    if buffer.samples.is_empty() {
        bail!("input file contains no audio samples: {}", input.display());
    }
    let source_rate = buffer.sample_rate;
    let resampled = buffer.into_mono_16k();

    let samples = if normalize {
        sample::normalize_peak(&resampled, NORMALIZE_TARGET_PEAK)
    } else {
        resampled
    };

    let output = output.unwrap_or_else(|| default_transcode_output(input));
    wav::write_wav_i16_mono(&output, TARGET_RATE, &samples)?;

    let duration_secs = samples.len() as u64 / TARGET_RATE as u64;
    let size_bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);

    Ok(TranscodeResult {
        output_path: output.to_string_lossy().into_owned(),
        sample_rate: TARGET_RATE,
        source_rate,
        duration_secs,
        size_bytes,
        normalized: normalize,
    })
}

/// Default output path: sibling of `input` with a `.16k.wav` extension.
pub fn default_transcode_output(input: &Path) -> PathBuf {
    let stem = input.file_stem().map(|s| s.to_owned()).unwrap_or_default();
    let mut name = stem;
    name.push(".16k.wav");
    input.with_file_name(name)
}

/// Port the desktop app's local IPC server listens on.
///
/// Kept in sync by hand with `src-tauri/src/ipc/server.rs::IPC_PORT` - the
/// two crates do not share a dependency edge, so this is a plain duplicate
/// constant rather than an import.
const IPC_PORT: u16 = 1424;

/// The message shown whenever the desktop app is not reachable at all: not
/// running, or running but its IPC listener crashed (a stale `"running"`
/// state file combined with a refused connection - see [`down_reason_message`]).
/// Both the CLI's stderr output and the MCP tool error surface this exact
/// wording so agents and humans get the same actionable guidance.
const APP_NOT_RUNNING_MESSAGE: &str =
    "The Echophrase desktop app must be running to transcribe. Start the app and try again.";

/// Shown when the app is running but the "MCP Server" setting is off.
const MCP_DISABLED_MESSAGE: &str = "Enable it in Settings > MCP Server in the Echophrase app.";

/// The `state` field of the desktop app's IPC state file
/// (`paths::ipc_state_file`).
#[derive(serde::Deserialize)]
struct IpcStateFile {
    state: String,
}

/// Read the desktop app's IPC state file and map it to the right
/// user-facing message. Missing, unreadable, unparseable, or `"running"` all
/// fall back to [`APP_NOT_RUNNING_MESSAGE`] - a `"running"` state combined
/// with a failed connection means the app crashed, so the generic message is
/// still the right one.
fn down_reason_message() -> &'static str {
    let Ok(contents) = std::fs::read_to_string(paths::ipc_state_file()) else {
        return APP_NOT_RUNNING_MESSAGE;
    };
    let Ok(parsed) = serde_json::from_str::<IpcStateFile>(&contents) else {
        return APP_NOT_RUNNING_MESSAGE;
    };
    match parsed.state.as_str() {
        "disabled" => MCP_DISABLED_MESSAGE,
        _ => APP_NOT_RUNNING_MESSAGE,
    }
}

/// Result of transcribing a WAV file via the desktop app's IPC server.
#[derive(Debug, Clone, Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct TranscribeResult {
    /// The transcribed text.
    pub text: String,
}

#[derive(serde::Deserialize)]
struct TranscribeErrorBody {
    error: String,
}

/// Send a WAV file to the running desktop app's IPC server for
/// transcription and return the transcript.
///
/// Requires the desktop app to be running (it owns the loaded transcription
/// model). If the token file is missing or the HTTP call cannot connect,
/// this returns a friendly message rather than a raw connection error -
/// [`down_reason_message`] distinguishes "app not running" from "app is
/// running" from "app is running, but the MCP Server setting is off" by reading the desktop app's IPC state
/// file (`paths::ipc_state_file`).
pub fn transcribe(input: &Path) -> Result<TranscribeResult> {
    let token_path = paths::ipc_token_file();
    let token =
        std::fs::read_to_string(&token_path).map_err(|_| anyhow::anyhow!(down_reason_message()))?;
    let token = token.trim();
    if token.is_empty() {
        bail!(down_reason_message());
    }

    let url = format!("http://127.0.0.1:{IPC_PORT}/v1/transcribe");
    let body = serde_json::json!({ "wav_path": input.to_string_lossy() });

    let response = ureq::post(&url)
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(body);

    match response {
        Ok(resp) => {
            let result: TranscribeResult = resp
                .into_json()
                .context("parsing transcribe response from desktop app")?;
            Ok(result)
        }
        Err(ureq::Error::Status(status, resp)) => {
            let message = resp
                .into_json::<TranscribeErrorBody>()
                .map(|body| body.error)
                .unwrap_or_else(|_| format!("desktop app returned HTTP {status}"));
            bail!(message)
        }
        Err(ureq::Error::Transport(_)) => {
            bail!(down_reason_message())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

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
    fn status_reports_idle_with_no_session() {
        let _home = isolated_home();
        let result = status().unwrap();
        assert!(!result.recording);
        assert!(result.session.is_none());
        clear_home();
    }

    #[test]
    #[serial]
    fn status_reports_active_session() {
        let _home = isolated_home();
        Session {
            pid: 999,
            output_path: "/tmp/foo.wav".into(),
            started_at: 1_700_000_000,
            foreground: false,
        }
        .save()
        .unwrap();

        let result = status().unwrap();
        assert!(result.recording);
        assert_eq!(result.session.unwrap().pid, 999);
        Session::clear().ok();
        clear_home();
    }

    #[test]
    #[serial]
    fn stop_errors_when_idle() {
        let _home = isolated_home();
        assert!(stop(5).is_err());
        clear_home();
    }

    #[test]
    fn default_transcode_output_appends_16k_suffix() {
        let input = PathBuf::from("/tmp/meeting.wav");
        assert_eq!(
            default_transcode_output(&input),
            PathBuf::from("/tmp/meeting.16k.wav")
        );
    }

    #[test]
    #[serial]
    fn transcribe_errors_with_friendly_message_when_no_token_file() {
        let _home = isolated_home();
        let err = transcribe(Path::new("/tmp/whatever.wav")).unwrap_err();
        assert_eq!(err.to_string(), APP_NOT_RUNNING_MESSAGE);
        clear_home();
    }

    #[test]
    #[serial]
    fn transcribe_errors_with_friendly_message_when_app_not_listening() {
        let home = isolated_home();
        // A token file exists but nothing is listening on IPC_PORT - same
        // user-facing message as "app not running", since from the CLI's
        // point of view both cases mean "can't reach the app".
        paths::ensure_dirs().unwrap();
        std::fs::write(paths::ipc_token_file(), "some-token").unwrap();
        let err = transcribe(Path::new("/tmp/whatever.wav")).unwrap_err();
        assert_eq!(err.to_string(), APP_NOT_RUNNING_MESSAGE);
        drop(home);
        clear_home();
    }

    #[test]
    #[serial]
    fn transcribe_errors_with_disabled_message_when_state_file_says_disabled() {
        let home = isolated_home();
        paths::ensure_dirs().unwrap();
        std::fs::write(paths::ipc_state_file(), r#"{"state":"disabled"}"#).unwrap();
        let err = transcribe(Path::new("/tmp/whatever.wav")).unwrap_err();
        assert_eq!(err.to_string(), MCP_DISABLED_MESSAGE);
        drop(home);
        clear_home();
    }

    #[test]
    #[serial]
    fn transcribe_ignores_legacy_not_pro_state_and_falls_back_to_generic_message() {
        let home = isolated_home();
        paths::ensure_dirs().unwrap();
        // A state file written by an older app build may still say `not_pro`.
        // It is no longer a known state, so the generic message applies.
        std::fs::write(paths::ipc_token_file(), "some-token").unwrap();
        std::fs::write(paths::ipc_state_file(), r#"{"state":"not_pro"}"#).unwrap();
        let err = transcribe(Path::new("/tmp/whatever.wav")).unwrap_err();
        assert_eq!(err.to_string(), APP_NOT_RUNNING_MESSAGE);
        drop(home);
        clear_home();
    }

    #[test]
    #[serial]
    fn transcribe_falls_back_to_generic_message_when_state_file_says_running() {
        let home = isolated_home();
        // A "running" state combined with a refused connection means the
        // app crashed - the generic "app not running" message is correct.
        paths::ensure_dirs().unwrap();
        std::fs::write(paths::ipc_state_file(), r#"{"state":"running"}"#).unwrap();
        let err = transcribe(Path::new("/tmp/whatever.wav")).unwrap_err();
        assert_eq!(err.to_string(), APP_NOT_RUNNING_MESSAGE);
        drop(home);
        clear_home();
    }

    #[test]
    #[serial]
    fn transcribe_falls_back_to_generic_message_when_state_file_is_unparseable() {
        let home = isolated_home();
        paths::ensure_dirs().unwrap();
        std::fs::write(paths::ipc_state_file(), "not json").unwrap();
        let err = transcribe(Path::new("/tmp/whatever.wav")).unwrap_err();
        assert_eq!(err.to_string(), APP_NOT_RUNNING_MESSAGE);
        drop(home);
        clear_home();
    }
}
