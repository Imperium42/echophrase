// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Window management commands

/// Terminate the app from the frontend (Quit button).
///
/// Routes through `shutdown::force_exit`, the same path the tray menu's Quit
/// uses. `@tauri-apps/plugin-process` `exit()` is NOT usable here: it goes
/// through `app.exit`, which deadlocks in this app (see `lib.rs` window-close
/// handler), and the capability set only grants `process:allow-restart`, so
/// the call was silently denied and the button did nothing (v0.9.3).
#[tauri::command]
pub fn quit_app() {
    log::info!("Quit requested from frontend");
    crate::shutdown::force_exit(0);
}

#[cfg(any(feature = "devtools", debug_assertions))]
use tauri::AppHandle;

/// Toggle devtools for the main window (dev + staging builds only)
///
/// This is a workaround for Tauri 2.0 beta bug #8771 where Ctrl+Shift+I doesn't work.
/// Opens devtools if closed, closes them if open.
///
/// Compiled when the `devtools` feature is enabled (staging) OR in debug builds (dev mode).
/// Production builds don't include this command at all.
#[cfg(any(feature = "devtools", debug_assertions))]
#[tauri::command]
pub async fn toggle_devtools(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
        log::info!("Devtools opened for main window");
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

/// Show the splash window on demand for ~10s, then close it.
///
/// Dev/staging only, driven by the Debug panel's "Splash" button. The splash
/// normally only exists for the brief window between process start and the
/// frontend calling `show_main_window`, which makes it nearly impossible to
/// inspect. This reuses `setup::splash::create_splash_window`, so it renders
/// the real splash rather than a lookalike.
///
/// Re-invoking while one is open closes the existing splash first, so the
/// button stays idempotent instead of erroring on the duplicate label.
#[cfg(any(feature = "devtools", debug_assertions))]
static DEBUG_SPLASH_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(any(feature = "devtools", debug_assertions))]
#[tauri::command]
pub async fn debug_show_splash(app: AppHandle) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    use tauri::Manager;

    // Each invocation gets a generation; the auto-close timer only fires for its
    // own generation, so re-clicking within 10s cannot close the newer splash.
    let generation = DEBUG_SPLASH_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;

    if let Some(existing) = app.get_webview_window("splash") {
        let _ = existing.close();
        // Let the label free up before rebuilding under the same name.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    }

    crate::setup::splash::create_splash_window(&app)?;
    log::info!("Debug: splash window shown for 10s");

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        if DEBUG_SPLASH_GENERATION.load(Ordering::SeqCst) != generation {
            return; // A newer splash replaced this one; its own timer owns it.
        }
        if let Some(splash) = handle.get_webview_window("splash") {
            let _ = splash.close();
            log::info!("Debug: splash window auto-closed after 10s");
        }
    });

    Ok(())
}
