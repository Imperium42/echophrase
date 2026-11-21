// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Telemetry module - privacy-first analytics via Aptabase (self-hosted)
//!
//! Uses a thin HTTP client that talks directly to the Aptabase ingest API,
//! replacing the abandoned `tauri-plugin-aptabase` (which panics on startup).
//!
//! Events are queued in memory and flushed in batches, but only while the app
//! is idle (no recording/transcription recently), on settings save, and on exit.
//! All events are anonymous (no user ID, no IP storage).
//! Users can opt out via Settings > Data > "Help improve Echophrase".
//!
//! See TELEMETRY.md for the full list of tracked events.
//!
//! Telemetry only exists in official builds (`--features official`, enabled by
//! the release workflow, which also supplies `APTABASE_APP_KEY`). Without that
//! feature the Aptabase client is not even compiled: every public function in
//! this module keeps its signature but does nothing, and no marker files are
//! written.

#[cfg(feature = "official")]
mod client;

use crate::settings::AppSettings;
use serde_json::json;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// True only in official builds. Community builds never report anything.
const OFFICIAL_BUILD: bool = cfg!(feature = "official");

/// Ensures `gpu_backend` telemetry fires only once per session (on first transcription).
static GPU_BACKEND_TRACKED: AtomicBool = AtomicBool::new(false);

/// The real Aptabase client, compiled only into official builds.
#[cfg(feature = "official")]
mod sink {
    use super::client::AptabaseClient;
    use std::sync::OnceLock;

    /// Global Aptabase client instance, initialized once during app setup.
    static APTABASE_CLIENT: OnceLock<AptabaseClient> = OnceLock::new();

    /// Initialize the Aptabase client and start the background flush loop.
    pub fn init(app: &tauri::AppHandle) {
        // Dev/debug builds send NO telemetry: without this, every `bun run tauri:dev`
        // session inflates daily-active and session counts with the developer's own
        // activity. Leaving the client uninitialized makes every track() a no-op.
        if cfg!(debug_assertions) {
            log::info!("Telemetry disabled: debug build (dev sessions are not counted)");
            return;
        }

        let app_version = app
            .config()
            .version
            .clone()
            .unwrap_or_else(|| "0.0.0".to_string());

        let client = AptabaseClient::new(
            env!("APTABASE_APP_KEY"),
            "https://telemetry.echophrase.com",
            &app_version,
        );

        if APTABASE_CLIENT.set(client).is_err() {
            log::warn!("Telemetry client already initialized");
            return;
        }

        // Start the background flush loop (only sends while idle - see client.rs)
        if let Some(client) = APTABASE_CLIENT.get() {
            client.start_flush_loop();
        }
    }

    /// Mark transcription activity so the flush loop stays quiet until idle.
    pub fn mark_activity() {
        if let Some(client) = APTABASE_CLIENT.get() {
            client.mark_activity();
        }
    }

    /// Flush queued events (async, non-blocking).
    pub fn flush() {
        if let Some(client) = APTABASE_CLIENT.get() {
            client.flush();
        }
    }

    /// Flush queued events synchronously (app exit only).
    pub fn flush_blocking() {
        if let Some(client) = APTABASE_CLIENT.get() {
            client.flush_blocking();
        }
    }

    /// Queue one event; dropped (with a debug log) if the client is not initialized.
    pub fn track_event(event: &str, props: Option<serde_json::Value>) {
        if let Some(client) = APTABASE_CLIENT.get() {
            client.track_event(event, props);
        } else {
            log::debug!(
                "Telemetry event '{}' dropped: client not initialized",
                event
            );
        }
    }
}

/// Community builds: same surface, no client, nothing ever leaves the machine.
#[cfg(not(feature = "official"))]
mod sink {
    /// No-op: telemetry is not compiled into community builds.
    pub fn init(_app: &tauri::AppHandle) {
        log::info!("Telemetry disabled: community build (no `official` feature)");
    }

    /// No-op.
    pub fn mark_activity() {}

    /// No-op.
    pub fn flush() {}

    /// No-op.
    pub fn flush_blocking() {}

    /// No-op.
    pub fn track_event(_event: &str, _props: Option<serde_json::Value>) {}
}

/// Initialize the Aptabase client and start the background flush loop.
/// Call once in Tauri's `.setup()` closure. No-op in community builds.
pub fn init(app: &tauri::AppHandle) {
    sink::init(app);
}

/// Mark transcription activity so the background flush loop stays quiet until
/// the app has been idle for a while (privacy: no telemetry request should be
/// observable during or right after a recording/transcription). Call on
/// recording start and transcription completion.
pub fn mark_activity() {
    sink::mark_activity();
}

/// Flush all queued events immediately (async, non-blocking).
/// Call on settings save or other natural sync points.
pub fn flush() {
    sink::flush();
}

/// Flush all queued events synchronously (blocking).
/// Call only on app exit where async tasks may not complete.
pub fn flush_blocking() {
    sink::flush_blocking();
}

/// Environment kill switch: honors the cross-vendor `DO_NOT_TRACK=1` convention
/// plus our own `ECHOPHRASE_NO_TELEMETRY`. Any value other than "0"/""/"false"
/// counts as "on", so `ECHOPHRASE_NO_TELEMETRY=1` and a bare `=true` both work.
/// Checked before every event, including the ones that predate user settings.
fn kill_switch_active() -> bool {
    ["DO_NOT_TRACK", "ECHOPHRASE_NO_TELEMETRY"]
        .iter()
        .any(|key| match std::env::var(key) {
            Ok(value) => {
                let v = value.trim().to_ascii_lowercase();
                !(v.is_empty() || v == "0" || v == "false")
            }
            Err(_) => false,
        })
}

/// Check if telemetry is enabled in user settings (and not killed by the environment).
fn is_enabled() -> bool {
    if !OFFICIAL_BUILD || kill_switch_active() {
        return false;
    }
    AppSettings::load()
        .map(|s| s.telemetry_enabled)
        .unwrap_or(true)
}

/// Track an event with optional JSON properties, respecting the user's opt-out preference.
///
/// Events are queued, not sent immediately. Failures are silently logged.
pub fn track(_app: &tauri::AppHandle, event: &str, props: Option<serde_json::Value>) {
    if !is_enabled() {
        return;
    }

    sink::track_event(event, props);
}

// ---------------------------------------------------------------------------
// Event-specific tracking functions
// ---------------------------------------------------------------------------

/// Whether this machine has never reported `first_launch` yet, i.e. the install
/// is fresh and onboarding still owes us a consent answer. Used by `setup()` to
/// decide between reporting immediately (upgrade: settings already on disk) and
/// deferring to `telemetry_report_first_run` (fresh: wait for consent).
pub fn is_first_launch_pending() -> bool {
    if !OFFICIAL_BUILD {
        return false;
    }
    match telemetry_marker_path(".first_launch_done") {
        Some(path) => !path.exists(),
        None => false,
    }
}

/// Report the very first launch of the app on this machine (fires once ever).
///
/// Respects the `telemetry_enabled` opt-out like every other event: on a fresh
/// install this is called only after onboarding has recorded the user's consent
/// choice. Debug builds send nothing (client is never initialized) and must not
/// write the marker file, so a later production install on the same machine
/// still counts. Same for a kill-switched run: bail before the marker write.
pub fn track_first_launch(app: &tauri::AppHandle) {
    if !OFFICIAL_BUILD || cfg!(debug_assertions) || kill_switch_active() {
        return;
    }
    let Some(path) = telemetry_marker_path(".first_launch_done") else {
        return;
    };
    if path.exists() {
        return;
    }

    track(
        app,
        "first_launch",
        Some(json!({ "arch": std::env::consts::ARCH })),
    );

    write_marker(&path, "1");
}

/// Whether this binary was built with the `official` feature (telemetry compiled in).
/// Community/source builds return false, and the UI hides the telemetry consent
/// controls, since nothing could ever be sent.
#[tauri::command]
pub fn is_official_build() -> bool {
    cfg!(feature = "official")
}

/// Report the deferred first-run event trio, invoked by the frontend once the
/// onboarding telemetry consent choice has been persisted. Idempotent: the
/// `.first_launch_done` marker and the `.daily_active_ts` 24h window dedup
/// repeat calls, so calling this defensively costs nothing.
#[tauri::command]
pub async fn telemetry_report_first_run(app: tauri::AppHandle) {
    track_first_launch(&app);
    track_app_updated(&app);
    track_daily_active(&app);
}

/// Report an in-place app update: fires when the stored last-seen version
/// differs from the running version. Sends `{ from, to }`. Respects the
/// opt-out (goes through `track()`); the version file is still updated even
/// when disabled so re-enabling later doesn't produce a stale `from`.
pub fn track_app_updated(app: &tauri::AppHandle) {
    if !OFFICIAL_BUILD || cfg!(debug_assertions) {
        return;
    }
    let Some(path) = telemetry_marker_path(".last_version") else {
        return;
    };
    let current = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| "0.0.0".to_string());

    let stored = std::fs::read_to_string(&path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    match stored {
        Some(prev) if prev != current => {
            track(
                app,
                "app_updated",
                Some(json!({ "from": prev, "to": current })),
            );
            write_marker(&path, &current);
        }
        None => {
            // First run with this feature (or fresh install): just seed the file.
            write_marker(&path, &current);
        }
        Some(_) => {} // Same version, nothing to do.
    }
}

/// Report a daily active user event (fires at most once per 24 hours).
///
/// More useful than raw app-launch counts since a single user can launch
/// the app many times a day. Persists the last-tracked timestamp to disk.
pub fn track_daily_active(app: &tauri::AppHandle) {
    // Debug builds must not touch the .daily_active_ts file either: it lives in the
    // shared config dir, so a dev run would suppress a standalone install's
    // daily_active on the same machine for up to 24h.
    if !OFFICIAL_BUILD || cfg!(debug_assertions) {
        return;
    }
    if !should_track_daily() {
        return;
    }

    let recording_mode = AppSettings::load()
        .ok()
        .and_then(|s| s.recording_mode)
        .unwrap_or_else(|| "push_to_talk".to_string());

    track(
        app,
        "daily_active",
        Some(json!({
            "recording_mode": recording_mode,
            "arch": std::env::consts::ARCH,
        })),
    );

    save_daily_timestamp();
}

/// Report current GPU backend once per session, on first transcription.
///
/// Called from the transcription path rather than app launch - knowing the GPU
/// backend is only relevant when the user actually records something.
pub fn track_gpu_backend(app: &tauri::AppHandle) {
    if GPU_BACKEND_TRACKED.swap(true, Ordering::Relaxed) {
        return; // Already tracked this session
    }
    track(
        app,
        "gpu_backend",
        Some(json!({
            "backend": detect_gpu_backend(),
            "vram_bucket": vram_bucket(),
        })),
    );
}

/// Track a completed transcription with context about the environment.
pub fn track_transcription(
    app: &tauri::AppHandle,
    model: &str,
    transform_used: bool,
    gpu_backend: &str,
    processing_ms: u64,
) {
    let settings = AppSettings::load().ok();

    let paste_at_cursor = settings
        .as_ref()
        .map(|s| s.output_to.contains(&"paste_at_cursor".to_string()))
        .unwrap_or(false);

    let remove_filler_words = settings
        .as_ref()
        .map(|s| s.remove_filler_words)
        .unwrap_or(false);

    track(
        app,
        "transcription_completed",
        Some(json!({
            "model": model,
            "transform_used": transform_used,
            "gpu_backend": gpu_backend,
            "paste_at_cursor": paste_at_cursor,
            "remove_filler_words": remove_filler_words,
            "processing_ms": processing_ms,
        })),
    );
}

/// Categorize a transcription error into a privacy-safe bucket.
/// Returns a static string label suitable for telemetry (no user data leaked).
pub fn categorize_transcription_error(error_msg: &str) -> &'static str {
    let lower = error_msg.to_lowercase();
    // Model-related checks first (before generic "not found" which would shadow them).
    // Covers: "Model directory not found", "No ONNX file found in", "No model file found in",
    // "No snapshots found in", "Unknown ONNX model ID", "Unknown model:", "download a model"
    if lower.contains("model")
        && (lower.contains("not found")
            || lower.contains("missing")
            || lower.contains("download")
            || lower.contains("unknown"))
    {
        "model_not_found"
    } else if lower.contains("no") && lower.contains("found in") && !lower.contains("tensor") {
        // "No ONNX file found in /path", "No snapshots found in /path"
        // but NOT "No output tensor found" (that's a runtime inference error)
        "model_not_found"
    } else if lower.contains("not found") || lower.contains("no such file") {
        "file_not_found"
    } else if lower.contains("out of memory") || lower.contains("oom") || lower.contains("alloc") {
        "oom"
    } else if lower.contains("onnx runtime")
        || lower.contains("ort not loaded")
        || lower.contains("not available")
    {
        "runtime_unavailable"
    } else {
        "backend_error"
    }
}

/// Track a failed transcription with error category (no raw error text for privacy).
pub fn track_transcription_failed(
    app: &tauri::AppHandle,
    model: &str,
    gpu_backend: &str,
    error_category: &str,
) {
    track(
        app,
        "transcription_failed",
        Some(json!({
            "model": model,
            "gpu_backend": gpu_backend,
            "error_category": error_category,
        })),
    );
}

/// Track when the user changes their model selection.
pub fn track_model_changed(app: &tauri::AppHandle, model: &str) {
    track(
        app,
        "model_changed",
        Some(json!({
            "model": model
        })),
    );
}

/// Track a snapshot of user preferences when settings are saved.
pub fn track_settings_saved(app: &tauri::AppHandle, settings: &AppSettings) {
    let recording_mode = settings
        .recording_mode
        .clone()
        .unwrap_or_else(|| "push_to_talk".to_string());

    track(
        app,
        "settings_saved",
        Some(json!({
            "output_to": settings.output_to,
            "recording_mode": recording_mode,
            "remove_filler_words": settings.remove_filler_words,
            "use_numeric_digits": settings.use_numeric_digits,
            "append_space_incomplete": settings.append_space_incomplete,
            "transform_enabled": settings.transform_enabled,
            "language": settings.language,
        })),
    );
}

/// Fire a final telemetry event when the user opts out, then cease tracking.
pub fn track_telemetry_disabled(app: &tauri::AppHandle) {
    // Bypass the is_enabled() check since we need to send this one last ping
    sink::track_event("telemetry_disabled", None);
    // Suppress unused variable warning - app handle kept for API consistency
    let _ = app;
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Detect the active GPU backend for this platform.
fn detect_gpu_backend() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        return "metal";
    }

    #[cfg(target_os = "windows")]
    {
        return "directml";
    }

    #[cfg(target_os = "linux")]
    {
        return "cpu";
    }

    #[allow(unreachable_code)]
    "cpu"
}

/// Bucket the detected GPU VRAM into a coarse, privacy-safe range label.
/// Returns "unknown" when no GPU/VRAM can be probed (e.g. CPU-only machines).
fn vram_bucket() -> &'static str {
    match crate::gpu_probe::primary_gpu_info() {
        Some((_, vram_mb)) if vram_mb < 4096 => "<4GB",
        Some((_, vram_mb)) if vram_mb < 8192 => "4-8GB",
        Some((_, vram_mb)) if vram_mb < 16384 => "8-16GB",
        Some(_) => ">16GB",
        None => "unknown",
    }
}

/// Report a panic from the release-build panic hook. Sends only a repo-relative
/// source location and thread name - never the panic message, which could
/// contain user paths or data. The caller is responsible for flushing.
/// Only referenced by the release-build panic hook, hence dead in debug.
#[cfg_attr(debug_assertions, allow(dead_code))]
pub fn track_app_panic(location: &str, thread: &str) {
    if !is_enabled() {
        return;
    }
    sink::track_event(
        "app_panic",
        Some(json!({ "location": location, "thread": thread })),
    );
}

/// Strip any absolute/user prefix from a panic location file path, keeping the
/// repo-relative tail (from the last `src/` component on). Prevents leaking
/// build-machine or user directory names in telemetry.
/// Only referenced by the release-build panic hook, hence dead in debug.
#[cfg_attr(debug_assertions, allow(dead_code))]
pub fn sanitize_panic_location(file: &str, line: u32) -> String {
    let normalized = file.replace('\\', "/");
    let tail = match normalized.rfind("/src/") {
        Some(idx) => &normalized[idx + 1..],
        None => normalized.rsplit('/').next().unwrap_or(&normalized),
    };
    format!("{tail}:{line}")
}

/// Path to a marker file in the shared echophrase config dir.
fn telemetry_marker_path(name: &str) -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("echophrase").join(name))
}

/// Best-effort write of a small marker file (parent dirs created as needed).
fn write_marker(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::File::create(path) {
        let _ = file.write_all(contents.as_bytes());
    }
}

/// Path to the file that stores the last daily_active timestamp.
fn daily_timestamp_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("echophrase").join(".daily_active_ts"))
}

/// Check if 24 hours have elapsed since the last daily_active event.
fn should_track_daily() -> bool {
    let Some(path) = daily_timestamp_path() else {
        return true;
    };
    if !path.exists() {
        return true;
    }

    let Ok(mut file) = std::fs::File::open(&path) else {
        return true;
    };
    let mut contents = String::new();
    if file.read_to_string(&mut contents).is_err() {
        return true;
    }

    let Ok(last_ts) = contents.trim().parse::<u64>() else {
        return true;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    now.saturating_sub(last_ts) >= 86400
}

/// Persist current timestamp so the next check knows when we last tracked.
fn save_daily_timestamp() {
    let Some(path) = daily_timestamp_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Ok(mut file) = std::fs::File::create(&path) {
        let _ = file.write_all(now.to_string().as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Env-var test: mutating the process environment is global, so this test owns
    /// both keys for its duration and clears them again on the way out. It is the
    /// only test in the crate that touches these vars, so there is no race with
    /// the parallel test runner (no `serial_test` dependency needed).
    #[test]
    fn kill_switch_honors_dnt_and_vendor_var() {
        // SAFETY: single-threaded with respect to these two vars - no other test reads
        // or writes them, and both are removed before this test returns.
        unsafe {
            std::env::remove_var("DO_NOT_TRACK");
            std::env::remove_var("ECHOPHRASE_NO_TELEMETRY");
            assert!(!kill_switch_active(), "unset means tracking allowed");

            std::env::set_var("DO_NOT_TRACK", "1");
            assert!(kill_switch_active(), "DO_NOT_TRACK=1 kills telemetry");

            // Explicit opt-in values are not a kill switch.
            std::env::set_var("DO_NOT_TRACK", "0");
            assert!(!kill_switch_active(), "DO_NOT_TRACK=0 is not a kill switch");
            std::env::set_var("DO_NOT_TRACK", "false");
            assert!(
                !kill_switch_active(),
                "DO_NOT_TRACK=false is not a kill switch"
            );
            std::env::set_var("DO_NOT_TRACK", "");
            assert!(!kill_switch_active(), "an empty value is not a kill switch");

            // The vendor-specific var works on its own, and any truthy value counts.
            std::env::remove_var("DO_NOT_TRACK");
            std::env::set_var("ECHOPHRASE_NO_TELEMETRY", "true");
            assert!(
                kill_switch_active(),
                "ECHOPHRASE_NO_TELEMETRY=true kills telemetry"
            );

            std::env::remove_var("ECHOPHRASE_NO_TELEMETRY");
            assert!(!kill_switch_active(), "cleanup restores the default");
        }
    }

    #[test]
    fn sanitize_panic_location_strips_user_prefixes() {
        assert_eq!(
            sanitize_panic_location("/home/alice/proj/src-tauri/src/lib.rs", 42),
            "src/lib.rs:42"
        );
        assert_eq!(
            sanitize_panic_location("C:\\Users\\bob\\app\\src\\main.rs", 7),
            "src/main.rs:7"
        );
        // Nested src dirs keep only the last one (closest to the file).
        assert_eq!(
            sanitize_panic_location("/x/src/deep/src/foo.rs", 1),
            "src/foo.rs:1"
        );
        // No src/ component: fall back to the bare file name.
        assert_eq!(
            sanitize_panic_location("/rustc/abc123/library/core/panicking.rs", 99),
            "panicking.rs:99"
        );
    }
}
