// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use tauri::Manager;

/// Show the main window and close the splash screen.
/// Called by the frontend once app initialization completes.
#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        // Position the main window on the splash's CENTER (the splash tracks a
        // user drag). Align centers, not top-left corners: the main window is
        // larger than the splash, so corner-matching pushed it down-and-right of
        // where the splash sat - on a centered splash that landed the window low
        // and toward the bottom of the screen. Falls back to centering if the
        // splash is gone or its geometry can't be read.
        let placed = if let Some(splash) = app.get_webview_window("splash") {
            match (
                splash.outer_position(),
                splash.outer_size(),
                main.outer_size(),
            ) {
                (Ok(sp), Ok(ss), Ok(ms)) => {
                    let cx = sp.x + ss.width as i32 / 2;
                    let cy = sp.y + ss.height as i32 / 2;
                    let target = tauri::PhysicalPosition::new(
                        cx - ms.width as i32 / 2,
                        cy - ms.height as i32 / 2,
                    );
                    log::debug!(
                        "[Window] Splash center: ({cx}, {cy}), placing main at ({}, {})",
                        target.x,
                        target.y
                    );
                    main.set_position(tauri::Position::Physical(target)).is_ok()
                }
                _ => false,
            }
        } else {
            false
        };
        if !placed {
            log::debug!("[Window] Centering main window");
            let _ = main.center();
        }

        if let Ok(main_pos) = main.outer_position() {
            log::debug!(
                "[Window] Main position after set: ({}, {})",
                main_pos.x,
                main_pos.y
            );
        }

        // unminimize() before show(): on WSLg (and some X11/Wayland WMs) a window created
        // hidden can surface in a minimized state that show()+set_focus() alone won't
        // restore, leaving the app stuck in the taskbar. Explicitly un-minimizing fixes it.
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();

        // Workaround: Tauri 2 + tao CSD bug on Linux/Wayland (tao#1046, tauri#11856).
        // Title bar buttons (minimize/maximize/close) are unresponsive after a
        // visible(false)->show() cycle. Toggling resizability fires
        // connect_resizable_notify on the HeaderBar, forcing GTK to recalculate
        // CSD hit regions. Must be across separate GTK idle cycles.
        // Pin the window size before/after to prevent visible bounce.
        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::GtkWindowExt;
            if let Ok(gtk_win) = main.gtk_window() {
                let (w, h) = gtk_win.size();
                let was_resizable = gtk_win.is_resizable();
                gtk_win.set_resizable(!was_resizable);
                glib::idle_add_local_once(move || {
                    gtk_win.set_resizable(was_resizable);
                    gtk_win.resize(w, h);
                });
            }
        }
    }
    if let Some(splash) = app.get_webview_window("splash") {
        let _ = splash.close();
    }
    // No temp file to clean up: the splash is now served from an in-memory
    // data: URL (see setup/splash.rs), not a file written to disk.
}
