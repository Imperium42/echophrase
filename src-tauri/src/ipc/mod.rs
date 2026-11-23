// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Localhost-only IPC server so the CLI (`echophrase transcribe`) and the
//! MCP `transcribe` tool can drive the desktop app's already-loaded
//! transcription model without a second copy of it.
//!
//! Lifecycle: [`spawn`] stores the transcribe closure + app version once, then
//! starts the listener only if the "MCP Server" setting is on. [`start`]/[`stop`] are idempotent and let the
//! settings command / patch-reconcile path flip the listener on the fly
//! without restarting the app - [`start`] generates a fresh bearer token,
//! writes it to the per-user token file both this crate ([`paths`]) and the
//! CLI (`cli/src/paths.rs::ipc_token_file`) resolve identically, then serves
//! the [`server::router`] on `127.0.0.1:IPC_PORT` with graceful shutdown wired
//! to a oneshot channel; [`stop`] fires that channel (closing the socket) and
//! best-effort removes the token file so a stale token never outlives the
//! listener it authenticates.
//!
//! Alongside the token file, [`spawn`]/[`start`]/[`stop`] maintain a small
//! JSON state file ([`paths::ipc_state_file`]) recording *why* the listener
//! isn't serving requests (`disabled`) or that it is (`running`).
//! The CLI/MCP `transcribe` front ends read this file on a connection
//! failure to turn "app not running" into a specific, actionable message -
//! see [`DownReason`].

pub mod paths;
pub mod server;

use std::io;
use std::sync::{Mutex, OnceLock};

use rand::RngCore;
use server::TranscribeFn;
use tokio::sync::oneshot;

pub use server::IPC_PORT;

/// Transcribe closure + app version, captured once at startup by [`spawn`]
/// and reused by every later [`start`] call (settings toggle, patch import).
static IPC_CONFIG: OnceLock<(TranscribeFn, &'static str)> = OnceLock::new();

/// Shutdown sender for the currently running listener, `None` when stopped.
/// Firing it closes the socket via `axum::serve(...).with_graceful_shutdown`.
static IPC_HANDLE: Mutex<Option<oneshot::Sender<()>>> = Mutex::new(None);

/// Store the transcribe config unconditionally, then start the listener only
/// if the "MCP Server" setting is enabled.
///
/// A blocked startup writes a [`DownReason`] to the state file so the CLI/MCP
/// can explain *why* instead of just "app not running". A bind failure is
/// logged and must never prevent the rest of the app from starting (the CLI
/// simply reports "app not reachable" until fixed).
pub fn spawn(transcribe: TranscribeFn, app_version: &'static str) {
    if IPC_CONFIG.set((transcribe, app_version)).is_err() {
        log::warn!("ipc: spawn called more than once, ignoring");
    }

    let enabled = crate::settings::AppSettings::get().mcp_server_enabled;
    if !enabled {
        log::info!("ipc: MCP server setting is off, not starting");
        write_down_state(DownReason::Disabled);
        return;
    }

    if let Err(e) = start() {
        log::error!("ipc: failed to start on launch: {e}");
    }
}

/// Start the listener if it isn't already running. Idempotent: a second call
/// while already running is a no-op success. Requires the stored config from
/// [`spawn`] - returns `Err` if that hasn't happened yet (should not occur in
/// practice since `spawn` runs once at startup before any command can fire).
pub fn start() -> Result<(), String> {
    if IPC_HANDLE.lock().unwrap().is_some() {
        return Ok(());
    }

    let (transcribe, version) = IPC_CONFIG
        .get()
        .cloned()
        .ok_or_else(|| "ipc: not initialized".to_string())?;

    let token = write_fresh_token().map_err(|e| format!("ipc: failed to write token: {e}"))?;
    let (tx, rx) = oneshot::channel();

    // Bind synchronously (std, not tokio) so `start()` never needs to block
    // on an async runtime - it's called both from sync setup code and from
    // async Tauri commands, and the latter would panic if we tried
    // `tauri::async_runtime::block_on` from within an already-running task.
    let addr = format!("127.0.0.1:{IPC_PORT}");
    let std_listener = std::net::TcpListener::bind(&addr)
        .map_err(|e| format!("ipc: failed to bind {addr}: {e}"))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| format!("ipc: failed to set non-blocking: {e}"))?;

    *IPC_HANDLE.lock().unwrap() = Some(tx);

    log::info!("ipc: listening on {addr}");
    tauri::async_runtime::spawn(async move {
        // Adopt the std listener HERE, inside the spawned task: `from_std`
        // requires a Tokio runtime context, and `start()` is also called from
        // sync setup code (app boot with the toggle already on) where no
        // reactor is running - adopting there panics.
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(e) => {
                log::error!("ipc: failed to adopt listener: {e}");
                *IPC_HANDLE.lock().unwrap() = None;
                clear_token_best_effort();
                return;
            }
        };
        let router = server::router(token, version, transcribe);
        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await
        {
            log::error!("ipc: server exited with error: {e}");
        }
        log::info!("ipc: listener stopped");
    });

    write_state_best_effort("running");

    Ok(())
}

/// Stop the listener if running. Idempotent: a second call while already
/// stopped is a no-op. Fires the graceful-shutdown signal (closing the
/// socket), best-effort clears the token file, and records
/// [`DownReason::Disabled`] in the state file.
pub fn stop() {
    let sender = IPC_HANDLE.lock().unwrap().take();
    if let Some(tx) = sender {
        let _ = tx.send(());
    }
    clear_token_best_effort();

    write_down_state(DownReason::Disabled);
}

/// Generate a random 32-byte token (hex-encoded), write it to the token
/// file with owner-only permissions on unix, and return it.
fn write_fresh_token() -> io::Result<String> {
    paths::ensure_run_dir()?;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);

    let path = paths::ipc_token_file();

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        // Remove any old file first: `mode` only applies when the file is
        // created, so this keeps the token owner-only from the first byte
        // (no window where it is readable before a chmod).
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(token.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, &token)?;

    Ok(token)
}

/// Best-effort removal of the token file on shutdown. Called from
/// `shutdown::force_exit` (which bypasses normal Rust drop order, so this
/// must be invoked explicitly rather than relied on via `Drop`) and from
/// [`stop`].
pub fn clear_token_best_effort() {
    let path = paths::ipc_token_file();
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != io::ErrorKind::NotFound {
            log::warn!("ipc: failed to remove token file {path:?}: {e}");
        }
    }
}

/// Why the IPC listener is not currently serving requests. Written to the
/// state file ([`paths::ipc_state_file`]) so the CLI/MCP `transcribe`
/// front ends can turn a bare connection failure into a specific,
/// actionable message instead of a single generic "app not running" one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownReason {
    /// The "MCP Server" setting is off.
    Disabled,
}

impl DownReason {
    fn as_str(self) -> &'static str {
        match self {
            DownReason::Disabled => "disabled",
        }
    }
}

/// Best-effort write of `{"state": "<reason>"}` to the state file. Errors
/// are logged, not fatal - a failure to write this hint just means the CLI
/// falls back to the generic "app not running" message.
pub fn write_down_state(reason: DownReason) {
    write_state_best_effort(reason.as_str());
}

/// Best-effort write of `{"state": "<state>"}` to the state file. `state` is
/// `"running"` when the listener just started, or a [`DownReason::as_str`]
/// value otherwise.
fn write_state_best_effort(state: &str) {
    if let Err(e) = paths::ensure_run_dir() {
        log::warn!("ipc: failed to create run dir for state file: {e}");
        return;
    }
    let path = paths::ipc_state_file();
    let body = serde_json::json!({ "state": state }).to_string();
    if let Err(e) = std::fs::write(&path, body) {
        log::warn!("ipc: failed to write state file {path:?}: {e}");
    }
}

/// Best-effort removal of the state file. Missing/unreadable is treated by
/// the CLI the same as "app not running", so removing it on graceful exit
/// (alongside [`clear_token_best_effort`]) is sufficient - a stale
/// `"running"` state left behind by a hard crash is an acceptable gap: the
/// CLI's connection attempt will fail regardless, and "running" maps to the
/// same generic message a missing file would.
pub fn clear_state_best_effort() {
    let path = paths::ipc_state_file();
    if let Err(e) = std::fs::remove_file(&path) {
        if e.kind() != io::ErrorKind::NotFound {
            log::warn!("ipc: failed to remove state file {path:?}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_transcribe() -> TranscribeFn {
        std::sync::Arc::new(|_path| Box::pin(async move { Ok("hi".to_string()) }))
    }

    /// Points `ECHOPHRASE_HOME` and `ECHOPHRASE_CONFIG_DIR` at a sandbox dir
    /// and unsets them on drop.
    struct EnvGuard;

    impl EnvGuard {
        fn set(dir: &std::path::Path) -> Self {
            // SAFETY: only this serial lifecycle test mutates these vars.
            unsafe {
                std::env::set_var("ECHOPHRASE_HOME", dir);
                std::env::set_var(crate::settings::CONFIG_DIR_ENV, dir);
            }
            EnvGuard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: see `set`.
            unsafe {
                std::env::remove_var("ECHOPHRASE_HOME");
                std::env::remove_var(crate::settings::CONFIG_DIR_ENV);
            }
        }
    }

    /// Read back the `state` field of the state file, if present and parseable.
    fn read_state() -> Option<String> {
        let contents = std::fs::read_to_string(paths::ipc_state_file()).ok()?;
        let value: serde_json::Value = serde_json::from_str(&contents).ok()?;
        value.get("state")?.as_str().map(str::to_string)
    }

    /// `start`/`stop` exercised directly against `IPC_CONFIG`/`IPC_HANDLE`
    /// without going through `spawn` - these are process-wide
    /// statics, so tests run serially via a single test function to avoid
    /// port/state collisions across the test binary. `AppSettings`'s cache
    /// and the state file are process-wide too, so the state-file
    /// transitions for `spawn`/`start`/`stop` are folded in here as well.
    #[test]
    fn start_stop_lifecycle_is_idempotent() {
        // A running Echophrase (dev or installed, MCP toggle on) already owns
        // the fixed IPC port; that's an environment conflict, not a bug here.
        // Checked FIRST so a skipped run never touches any state.
        if std::net::TcpListener::bind(("127.0.0.1", server::IPC_PORT)).is_err() {
            eprintln!(
                "skipping: 127.0.0.1:{} is in use (is Echophrase running?)",
                server::IPC_PORT
            );
            return;
        }

        // Redirect settings.yaml and the IPC run dir (token + state file) into
        // a tempdir so this test can never touch the real user's files.
        let sandbox = tempfile::tempdir().unwrap();
        let _env = EnvGuard::set(sandbox.path());

        // Populate IPC_CONFIG once; ignore "already set" if another test in
        // this binary ran first (OnceLock is process-wide).
        let _ = IPC_CONFIG.set((ok_transcribe(), "9.9.9"));

        // Deterministic reason for the trailing `stop()` calls below.
        let _ = crate::settings::AppSettings::update(|s| s.mcp_server_enabled = false);

        // Ensure clean slate.
        stop();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let first = start();
        assert!(first.is_ok(), "first start should succeed: {first:?}");
        assert!(start().is_ok(), "second start should be a no-op success");

        // /v1/health should respond while running.
        let health = rt.block_on(async {
            reqwest::get(format!("http://127.0.0.1:{IPC_PORT}/v1/health"))
                .await
                .map(|r| r.status())
        });
        assert!(health.is_ok() && health.unwrap().is_success());
        assert!(paths::ipc_token_file().exists());
        assert_eq!(
            read_state(),
            Some("running".to_string()),
            "start() should overwrite any prior disabled state with running"
        );

        stop();
        stop(); // idempotent

        // Give the graceful shutdown a moment to actually close the socket.
        std::thread::sleep(std::time::Duration::from_millis(200));
        let after_stop = rt.block_on(async {
            reqwest::get(format!("http://127.0.0.1:{IPC_PORT}/v1/health")).await
        });
        assert!(after_stop.is_err(), "port should be closed after stop()");
        assert!(
            !paths::ipc_token_file().exists(),
            "token file should be removed"
        );
        assert_eq!(
            read_state(),
            Some("disabled".to_string()),
            "stop() should record `disabled` when the MCP Server setting is off"
        );

        // spawn() with the setting off records `disabled` and does not start.
        let _ = crate::settings::AppSettings::update(|s| s.mcp_server_enabled = false);
        clear_state_best_effort();
        spawn(ok_transcribe(), "9.9.9");
        assert_eq!(
            read_state(),
            Some("disabled".to_string()),
            "spawn() should write disabled when the MCP Server setting is off"
        );

        // Restore a clean slate for any other test in this binary.
        let _ = crate::settings::AppSettings::update(|s| s.mcp_server_enabled = false);
        clear_state_best_effort();
    }
}
