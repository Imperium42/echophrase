// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Centralised process shutdown.
//!
//! Every exit path in the app funnels through [`force_exit`]. This exists
//! because graceful Rust/libc shutdown deadlocks on Linux in this app:
//!
//! - `ort` / ONNX Runtime atexit handlers can deadlock during `dlclose()` of
//!   provider libraries (onnxruntime#21207).
//! - `webkit2gtk` can block on GTK main-loop teardown when a window was
//!   never fully realised.
//! - `tauri-plugin-updater` may still be holding a `reqwest` request when
//!   the process tries to join the tokio runtime.
//!
//! `libc::_exit(n)` bypasses atexit handlers, C++ static destructors, and
//! Rust drops entirely - it is a direct `SYS_exit_group` syscall on Linux
//! and the documented escape hatch for this deadlock class.
//!
//! Upstream tracking:
//! - onnxruntime#21207 (unresolved as of 2026-04)
//! - rust-lang/rust#126600 (process::exit not thread-safe with C atexit)

use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// Process-wide handle used by [`force_exit`] to perform minimal pre-exit
/// cleanup (specifically: unregister OS-level global shortcuts so that
/// a subsequent launch doesn't see them as stuck on the same keychord).
///
/// Populated once from `setup()` in `lib.rs`. If unset (e.g. crash before
/// setup completes), `force_exit` still fires - we just skip the cleanup.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Stash the AppHandle for use at exit time. Call this exactly once from
/// Tauri's `setup` closure. Subsequent calls are no-ops.
pub fn install_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// Terminate the process immediately with the given exit code.
///
/// Before exiting, best-effort unregisters all global shortcuts so that
/// the OS doesn't hold `Ctrl+Shift+A` (etc.) against our now-dead PID.
/// Without this, a second launch would fail with "HotKey already registered".
///
/// Does NOT run atexit handlers, C++ static destructors, Rust drops,
/// stdio flush, or Tauri's graceful shutdown. Safe to call from any
/// thread, including the GTK main thread inside a `WindowEvent`
/// callback.
///
/// Only stdio buffers may lose their tail - this app logs via
/// `tauri-plugin-log` which writes line-buffered to stderr, so the
/// final log line preceding this call is observable.
pub fn force_exit(code: i32) -> ! {
    log::info!("force_exit({}) - running minimal pre-exit cleanup", code);
    unregister_shortcuts_best_effort();

    // macOS: restore the dedicated Dictation (mic) key to system Dictation by
    // clearing our hidutil remap. Best-effort - the remap is cleared on reboot
    // regardless, and self-heals on next launch. (Prototype.)
    #[cfg(target_os = "macos")]
    crate::macos_dictation::revert_remap();
    log::info!(
        "force_exit({}) - bypassing atexit to avoid ORT teardown deadlock",
        code
    );

    // Also clear the single-instance lock file so a relaunch from the tray's
    // "Quit" doesn't trip the "Another instance already running" guard if the
    // OS hasn't released the file handle yet.
    clear_instance_lock_best_effort();

    // Remove the IPC bearer token so a stale token never outlives this
    // process; the next launch generates and writes a fresh one. Also clear
    // the "why is the server down" state file - a graceful exit means the
    // real reason is just "app not running", so let a missing state file
    // fall back to that generic message rather than a stale disabled
    // hint from before this exit.
    crate::ipc::clear_token_best_effort();
    crate::ipc::clear_state_best_effort();

    // On Windows, `libc::_exit` goes through MSVCRT which still runs
    // DLL_PROCESS_DETACH for every loaded DLL. ORT/DirectML/audio drivers
    // can hang in their detach handlers, leaving the process alive after
    // "Quit". TerminateProcess skips all DLL teardown and kills the process
    // immediately. Brutal but reliable - by this point we've already done
    // the only cleanup that matters (shortcut unregister + lock file).
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        // SAFETY: GetCurrentProcess returns a pseudo-handle that's always
        // valid for the current process; TerminateProcess with this handle
        // unconditionally ends the process. No Rust invariants can be
        // violated by a process that no longer exists.
        unsafe {
            TerminateProcess(GetCurrentProcess(), code as u32);
        }
        // Unreachable - TerminateProcess does not return.
        unsafe { libc::_exit(code) }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // SAFETY: `_exit` has no preconditions - it is a single syscall that
        // terminates the process. No Rust invariants can be violated by a
        // process that no longer exists.
        unsafe { libc::_exit(code) }
    }
}

/// Decide whether a Tauri run-loop event must end the process via
/// [`force_exit`]. Returns the exit code, or `None` to keep running.
///
/// `RunEvent::Exit` is tao's `LoopDestroyed`. Our own quit paths never get
/// here (they call `force_exit` directly). Tauri-initiated exits and
/// restarts (`relaunch()` after an update, `request_restart`) always send
/// `ExitRequested` first and restart only AFTER the `Exit` callback returns,
/// so those must be left alone - `exit_requested_seen` is true for them.
/// An `Exit` with no `ExitRequested` before it means something outside the
/// app tore the loop down - in practice Windows `WM_ENDSESSION` (logoff,
/// shutdown, or an installer's Restart Manager closing the app). tao 0.35
/// moves its runner to `Destroyed` but keeps pumping messages, and the next
/// event dispatched panics with "cannot move state from Destroyed"
/// (tao runner.rs:371, tauri-apps/tao#1310). Terminating inside this event
/// means there is no next event. tao 0.37 calls `std::process::exit` here
/// instead, which can still hang in ORT/DirectML DLL teardown, so this stays
/// useful after a tauri bump too. See KNOWN_ISSUES.md.
pub fn exit_code_for_run_event(event: &tauri::RunEvent, exit_requested_seen: bool) -> Option<i32> {
    match event {
        tauri::RunEvent::Exit if !exit_requested_seen => Some(0),
        _ => None,
    }
}

/// Try to delete the single-instance lock file so a relaunch doesn't see a
/// stale entry. Best-effort - errors are logged and swallowed because we are
/// about to exit anyway.
fn clear_instance_lock_best_effort() {
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    let Ok(local_dir) = app.path().app_local_data_dir() else {
        return;
    };
    let lock_path = local_dir.join("instance.lock");
    match std::fs::remove_file(&lock_path) {
        Ok(()) => log::info!("force_exit: removed instance lock {:?}", lock_path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!(
            "force_exit: could not remove instance lock {:?}: {}",
            lock_path,
            e
        ),
    }
}

/// Best-effort shortcut unregistration. Never panics, never blocks longer
/// than the plugin's own timeout. Failure is logged and swallowed - we
/// must exit either way.
fn unregister_shortcuts_best_effort() {
    let Some(app) = APP_HANDLE.get() else {
        log::warn!("force_exit: AppHandle not installed - skipping shortcut cleanup");
        return;
    };
    match app.global_shortcut().unregister_all() {
        Ok(()) => log::info!("force_exit: global shortcuts unregistered"),
        Err(e) => log::warn!(
            "force_exit: unregister_all failed (proceeding anyway): {}",
            e
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrequested_loop_exit_routes_through_force_exit() {
        // WM_ENDSESSION path: Exit arrives with no ExitRequested before it.
        assert_eq!(
            exit_code_for_run_event(&tauri::RunEvent::Exit, false),
            Some(0)
        );
    }

    #[test]
    fn requested_exit_or_restart_is_left_to_tauri() {
        // relaunch() after an update: ExitRequested, then Exit, then tauri
        // restarts the process. Force-exiting here would kill the relaunch.
        assert_eq!(exit_code_for_run_event(&tauri::RunEvent::Exit, true), None);
    }

    #[test]
    fn non_exit_events_do_not_exit() {
        for seen in [false, true] {
            assert_eq!(exit_code_for_run_event(&tauri::RunEvent::Ready, seen), None);
            assert_eq!(
                exit_code_for_run_event(&tauri::RunEvent::Resumed, seen),
                None
            );
            assert_eq!(
                exit_code_for_run_event(&tauri::RunEvent::MainEventsCleared, seen),
                None
            );
        }
    }
}
