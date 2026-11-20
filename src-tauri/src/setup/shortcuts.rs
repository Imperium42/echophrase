// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/// Register global shortcuts (dev mode only - F5 to reload webview).
pub fn register_shortcuts(app: &tauri::App) {
    #[allow(unused_imports)]
    use tauri::Manager;
    // Register F5 to reload webview (dev only - useful when Vite disconnects)
    #[cfg(debug_assertions)]
    {
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
        let shortcut: Shortcut = "F5".parse().unwrap();
        if let Err(e) = app.global_shortcut().register(shortcut) {
            log::warn!("Failed to register F5 reload shortcut: {}", e);
        } else {
            log::info!("F5 reload shortcut registered (dev mode)");
        }
    }
}
