// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/// Tauri managed-state handle for AppSettings.
///
/// There is exactly ONE in-memory copy of the settings: the process-wide
/// `SETTINGS_CACHE` inside `settings::mod`. This type does NOT hold a second
/// copy - it is a zero-sized marker that lets commands depend on settings via
/// Tauri's state system while `AppSettings::get`/`update` remain the single
/// access point.
///
/// An earlier version wrapped its own `Arc<RwLock<AppSettings>>`, which meant
/// two caches that could silently diverge: writes going through `save()`
/// refreshed the static one but not the managed one.
///
/// Hot-path callers (transcription, VAD loops, recording) call
/// `AppSettings::get()` - no disk I/O once warm.
use tauri::{AppHandle, Emitter, Manager};

use crate::settings::AppSettings;

/// Tauri managed state marker.
///
/// Register once via `app.manage(SettingsCache::init())`.
pub struct SettingsCache;

impl SettingsCache {
    /// Warm the settings cache and return the marker for `app.manage(...)`.
    pub fn init() -> SettingsCache {
        // Populates the static cache so the first command call is a cache hit.
        let _ = AppSettings::get();
        SettingsCache
    }

    /// Clone the current settings.
    #[allow(dead_code)]
    pub fn get(_app: &AppHandle) -> AppSettings {
        AppSettings::get()
    }

    /// Apply a partial mutation, persist, and broadcast `settings-changed`.
    ///
    /// Delegates to `AppSettings::update`, which holds the write lock across
    /// mutate + persist so concurrent writers cannot clobber each other's
    /// fields. Use this variant when an `AppHandle` is available and other
    /// parts of the app should react to the change.
    pub fn update<F: FnOnce(&mut AppSettings)>(app: &AppHandle, f: F) -> Result<(), String> {
        // Guard against being called before `app.manage()` ran, which would
        // otherwise silently succeed while listeners are not yet attached.
        if app.try_state::<SettingsCache>().is_none() {
            return Err("SettingsCache not registered".to_string());
        }

        AppSettings::update(f).map_err(|e| e.to_string())?;

        // Broadcast so other parts of the app (tray status, feature gate, the
        // frontend settings store) can react.
        let _ = app.emit("settings-changed", ());
        Ok(())
    }
}
