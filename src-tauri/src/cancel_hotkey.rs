// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Global Esc cancels an in-progress recording from any app.
//!
//! Esc is registered as a global shortcut ONLY while Recording and released
//! otherwise (Transcribing too: inference can't be stopped mid-flight), so the rest of the time it
//! reaches other apps untouched. Pattern credit: Handy (cjpais/Handy, MIT),
//! which registers its cancel shortcut the same way and skips Linux because
//! dynamic registration is unreliable there.
//!
//! A press emits `overlay-cancel` to the main window, reusing the exact discard
//! path of the overlay X button and the in-window Esc (`cancelRecording()`).

/// Called from `tray_status::set` on every status transition. No-op on Linux.
#[cfg(target_os = "linux")]
pub fn sync(_app: &tauri::AppHandle, _status: crate::tray_status::TrayStatus) {}

#[cfg(not(target_os = "linux"))]
pub use imp::sync;

#[cfg(not(target_os = "linux"))]
mod imp {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tauri::{AppHandle, Emitter};
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

    use crate::settings::AppSettings;
    use crate::tray_status::TrayStatus;

    /// Latest wanted state; the reconcile task always applies the newest value,
    /// so out-of-order task scheduling can't leave Esc stuck registered.
    static DESIRED: AtomicBool = AtomicBool::new(false);
    /// Whether we currently hold the registration. Serializes reconciles.
    static REGISTERED: Mutex<bool> = Mutex::new(false);

    fn escape() -> Shortcut {
        Shortcut::new(None, Code::Escape)
    }

    /// True when the user's own record hotkey is a bare Esc, which this would collide with.
    pub(super) fn hotkey_is_escape(hotkey: &str) -> bool {
        matches!(hotkey.trim().to_lowercase().as_str(), "escape" | "esc")
    }

    /// Whether the user's settings allow the global Esc during a recording.
    pub(super) fn wanted_for(settings: &AppSettings) -> bool {
        let push_to_talk = settings
            .recording_mode
            .as_deref()
            .is_none_or(|mode| mode == "push_to_talk");
        settings.hotkey_cancel_enabled && push_to_talk && !hotkey_is_escape(&settings.hotkey)
    }

    pub fn sync(app: &AppHandle, status: TrayStatus) {
        // Recording only: cancelling mid-transcription can't stop the in-flight
        // inference, so its text would still paste after a "cancelled" pill.
        // Push-to-talk only: open mic stays in Recording for the whole session,
        // so holding Esc would swallow it in every app until open mic stops.
        let want = status == TrayStatus::Recording && wanted_for(&AppSettings::get());
        DESIRED.store(want, Ordering::SeqCst);

        // Spawned, not inline: `tray_status::set` holds its lock here and the
        // shortcut plugin dispatches to the main thread (deadlock risk).
        let app = app.clone();
        // A plain thread: the plugin blocks waiting on the main thread, which
        // must not tie up an async worker.
        std::thread::spawn(move || reconcile(&app));
    }

    fn reconcile(app: &AppHandle) {
        let mut registered = REGISTERED.lock().unwrap_or_else(|e| e.into_inner());
        let want = DESIRED.load(Ordering::SeqCst);
        let shortcuts = app.global_shortcut();
        // Something else (eg the frontend's unregisterAll on webview init or
        // teardown) may have dropped it behind our back; trust the plugin.
        if *registered && !shortcuts.is_registered(escape()) {
            *registered = false;
        }
        if want == *registered {
            return;
        }
        if want {
            let result = shortcuts.on_shortcut(escape(), |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    log::info!("global Esc pressed - cancelling recording");
                    if let Err(e) = app.emit_to("main", "overlay-cancel", ()) {
                        log::warn!("emit overlay-cancel failed: {e}");
                    }
                }
            });
            match result {
                Ok(()) => *registered = true,
                Err(e) => log::warn!("register global Esc cancel failed: {e}"),
            }
        } else {
            if let Err(e) = shortcuts.unregister(escape()) {
                log::warn!("unregister global Esc cancel failed: {e}");
            }
            // A failed unregister keeps the plugin entry; mirror it so the
            // next reconcile retries instead of assuming it is gone.
            *registered = shortcuts.is_registered(escape());
        }
    }
}

#[cfg(all(test, not(target_os = "linux")))]
mod tests {
    use super::imp::{hotkey_is_escape, wanted_for};
    use crate::settings::AppSettings;

    #[test]
    fn wanted_only_for_push_to_talk_with_setting_on() {
        let mut settings = AppSettings::default();
        assert!(wanted_for(&settings), "default push-to-talk, setting on");
        settings.recording_mode = Some("open_mic".to_string());
        assert!(
            !wanted_for(&settings),
            "open mic would hijack Esc all session"
        );
        settings.recording_mode = Some("push_to_talk".to_string());
        settings.hotkey_cancel_enabled = false;
        assert!(!wanted_for(&settings), "setting off");
        settings.hotkey_cancel_enabled = true;
        settings.hotkey = "escape".to_string();
        assert!(!wanted_for(&settings), "record hotkey is Esc");
    }

    #[test]
    fn detects_escape_record_hotkey() {
        assert!(hotkey_is_escape("escape"));
        assert!(hotkey_is_escape(" Esc "));
        assert!(!hotkey_is_escape("ctrl+escape"));
        assert!(!hotkey_is_escape("f13"));
    }
}
