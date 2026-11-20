// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, TrayIconEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// Stable Windows identity for the tray icon (`NOTIFYICONDATA.guidItem`).
///
/// Windows keys the user's "always show in the taskbar" setting on the icon's
/// identity. Without a GUID that identity is the exe path, so any install
/// method that puts each version in its own folder (Scoop) resets the pin on
/// every update. With this GUID and an Authenticode-signed exe from the same
/// publisher, the setting survives path changes. Never change this value, and
/// never reuse it for a second icon.
#[cfg(windows)]
const TRAY_ICON_GUID: u128 = 0xe9bc630e_f4da_480e_bb4c_aa215a7b64a5;

/// Build and install the system tray icon with its context menu.
pub fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Must run before the icon is built; tauri's builder does not expose the
    // attribute, so the patched tray-icon crate reads a process-wide default.
    #[cfg(windows)]
    tray_icon::set_default_guid(TRAY_ICON_GUID);

    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
    // Label starts as "Start Recording" (idle state). tray_status::set() updates
    // it to "Stop Recording" whenever recording/transcribing begins, and back on stop.
    let toggle_item = MenuItem::with_id(
        app,
        "toggle_recording",
        "Start Recording",
        true,
        None::<&str>,
    )?;
    let menu = Menu::with_items(app, &[&show_item, &toggle_item, &quit_item])?;

    let tray = tauri::tray::TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Echophrase")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_tray_icon_event(|tray, event| {
            // Double-click restores the window from hidden/minimized state.
            // Left-click still opens the menu (show_menu_on_left_click), but a
            // double-click feels more discoverable for "bring the app back".
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "quit" => {
                    log::info!("Quit requested from tray menu");
                    crate::shutdown::force_exit(0);
                }
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "toggle_recording" => {
                    // Delegate to the frontend which owns the recording state machine
                    // (open-mic vs single-shot distinction, usage gating, etc.).
                    // The frontend emits the correct start or stop command and the
                    // tray label updates via tray_status::set once state changes.
                    log::info!("Toggle recording requested from tray menu");
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.emit("tray-toggle-recording", ());
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    if let Err(e) = crate::tray_status::install(&app.handle(), tray, toggle_item) {
        log::warn!("tray status indicator unavailable: {e}");
    }

    Ok(())
}

/// Send a native OS notification the first time the app is hidden to the tray.
#[cfg_attr(target_os = "linux", allow(dead_code))]
/// Subsequent calls are no-ops once the flag is persisted in settings.
/// Errors are logged and swallowed - a missing notification is not fatal.
pub fn maybe_send_tray_notice(app: &tauri::AppHandle) {
    if crate::settings::AppSettings::get().tray_close_notice_shown {
        return;
    }

    // Mark as shown before attempting to send - if the notification fails we
    // still flip the flag so we don't spam on every close attempt.
    if let Err(e) = crate::settings::AppSettings::update(|s| s.tray_close_notice_shown = true) {
        log::warn!("tray notice: could not save settings: {e}");
    }

    let result = app
        .notification()
        .builder()
        .title("Echophrase is still running")
        .body("Right-click the tray icon to quit.")
        .show();

    match result {
        Ok(_) => log::info!("tray notice: first-close notification sent"),
        Err(e) => log::warn!("tray notice: notification failed: {e}"),
    }
}
