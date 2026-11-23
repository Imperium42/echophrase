// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use crate::error::ToCmdErr;
use serde::{Deserialize, Serialize};
use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadStatus {
    Connecting,
    Downloading,
    VerifyingIntegrity,
    Complete,
}

// Global debug log to capture backend logs for frontend display
pub static DEBUG_LOGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

// Global download cancellation flag
pub static DOWNLOAD_CANCELLED: AtomicBool = AtomicBool::new(false);

// Serialises model downloads across BOTH the registry picker
// (download_model_with_progress) and BYO custom-model HF imports
// (import_from_hf) - both now share the same progress-event bus and the same
// DOWNLOAD_CANCELLED flag, so two overlapping downloads would corrupt each
// other's displayed progress and let one cancel silently kill the other.
pub static DOWNLOAD_SESSION_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Acquire [`DOWNLOAD_SESSION_LOCK`], rejecting a genuinely overlapping
/// download attempt but tolerating `switch_model_download`'s own
/// cancel-then-restart of the CURRENT download (which cancels the old
/// session and waits ~100ms before starting the new one - the old session's
/// guard may not have dropped yet at that instant). A short bounded retry
/// bridges that gap; a truly unrelated second download holds the lock for
/// its entire (many-second) duration, so it still fails out here.
pub async fn acquire_download_lock() -> Result<tokio::sync::MutexGuard<'static, ()>, &'static str> {
    for _ in 0..10 {
        if let Ok(guard) = DOWNLOAD_SESSION_LOCK.try_lock() {
            return Ok(guard);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err("A model download is already in progress. Wait for it to finish or cancel it first.")
}

// Serialises preloads. A duplicate caller (auto-detect swap racing the
// EP/auth-gated init preload, or an HMR page reload) WAITS on this instead of
// getting a bare `Ok(())` that is indistinguishable from "already loaded" -
// which is what made the frontend sample `preloadPhase` mid-load and log a
// bogus "preload did not complete" warning. Held across the load await, which
// is safe because `preload_with_events` is bounded by `PRELOAD_CEILING_SECS`.
static PRELOAD_LOCK: LazyLock<tokio::sync::Mutex<()>> =
    LazyLock::new(|| tokio::sync::Mutex::new(()));

// Model id currently resident in memory, or `None` after an unload. `std`
// (not tokio) so the synchronous `unload_model` command can clear it.
static LOADED_MODEL: Mutex<Option<String>> = Mutex::new(None);

/// Hard ceiling (seconds) for the background ONNX preload. Single source of
/// truth for the preload timeout - the JS net in `modelStore.ts`
/// (`PRELOAD_TIMEOUT_MS`) must stay above this with a buffer. With the DirectML
/// ORT#26858 workaround (`onnx_backend::build_parakeet_exec_config`) session
/// creation returns in seconds, so a real hang should fail fast rather than
/// after 10 minutes.
pub const PRELOAD_CEILING_SECS: u64 = 60;

pub fn log_debug(message: String) {
    log::info!("{}", message);
    if let Ok(mut logs) = DEBUG_LOGS.lock() {
        logs.push(message);
        if logs.len() > 50 {
            let excess = logs.len() - 50;
            logs.drain(0..excess);
        }
    }
}

// ---------------------------------------------------------------------------
// Debug logs
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_debug_logs() -> Result<Vec<String>, String> {
    if let Ok(logs) = DEBUG_LOGS.lock() {
        Ok(logs.clone())
    } else {
        Err("Failed to read debug logs".to_string())
    }
}

// ---------------------------------------------------------------------------
// Model existence checks
// ---------------------------------------------------------------------------

/// Checks if the currently selected model (from settings) is downloaded.
/// Uses the unified ModelFileChecker trait for format-agnostic detection.
#[tauri::command]
pub async fn check_selected_model_exists() -> Result<bool, String> {
    log_debug("🔍 Checking if selected model from settings exists...".to_string());

    let settings = match crate::settings::AppSettings::load() {
        Ok(s) => s,
        Err(e) => {
            log_debug(format!("⚠️ Failed to load settings: {}", e));
            return Ok(false);
        }
    };

    let model_id = &settings.model;
    log_debug(format!("📋 Selected model: {}", model_id));

    // BYO custom model: exists when the imported directory holds model.onnx.
    if model_id == "custom" {
        let model_exists = settings
            .custom_model_path
            .as_deref()
            .map(|dir| std::path::Path::new(dir).join("model.onnx").exists())
            .unwrap_or(false);
        log_debug(format!("🎯 Custom model imported: {}", model_exists));
        return Ok(model_exists);
    }

    use crate::models::get_model_info;
    match get_model_info(model_id) {
        Some(model) => {
            let downloaded = model.is_downloaded();
            if downloaded {
                log_debug(format!(
                    "✅ Model '{}' ({}) is downloaded",
                    model.id, model.name
                ));
            } else {
                log_debug(format!(
                    "❌ Model '{}' ({}) not found",
                    model.id, model.name
                ));
            }
            Ok(downloaded)
        }
        None => {
            log_debug(format!("⚠️ Model '{}' not in registry", model_id));
            Ok(false)
        }
    }
}

#[tauri::command]
pub async fn check_whisper_model_exists() -> Result<bool, String> {
    log_debug("🔍 Starting model existence check...".to_string());

    let home_dir = dirs::home_dir().ok_or_else(|| "Home directory not found".to_string())?;
    let cache_dir = home_dir.join(".cache").join("huggingface").join("hub");
    let model_dir = cache_dir
        .join("models--Systran--faster-whisper-small.en")
        .join("snapshots")
        .join("main");
    let our_model_file = model_dir.join("model.bin");
    let our_partial_file = model_dir.join("model.bin.partial");

    log_debug(format!(
        "🎯 Checking our download location: {:?}",
        our_model_file
    ));

    if our_model_file.exists() {
        let file_size = std::fs::metadata(&our_model_file)
            .map(|m| m.len())
            .unwrap_or(0);
        let size_mb = file_size as f64 / (1024.0 * 1024.0);
        log_debug(format!(
            "📊 Found complete model file! Size: {} bytes ({:.1} MB)",
            file_size, size_mb
        ));
        if size_mb > 400.0 {
            log_debug("✅ Complete model file size looks valid - model exists!".to_string());
            return Ok(true);
        } else {
            log_debug(format!(
                "❌ Complete model file too small ({:.1} MB) - likely corrupted, removing...",
                size_mb
            ));
            let _ = std::fs::remove_file(&our_model_file);
        }
    }

    if our_partial_file.exists() {
        let file_size = std::fs::metadata(&our_partial_file)
            .map(|m| m.len())
            .unwrap_or(0);
        let size_mb = file_size as f64 / (1024.0 * 1024.0);
        log_debug(format!(
            "📁 Found partial download: {} bytes ({:.1} MB) - ready for resume",
            file_size, size_mb
        ));
        return Ok(false);
    }

    let home_dir = dirs::home_dir().ok_or_else(|| "Home directory not found".to_string())?;
    let echophrase_models = dirs::data_dir()
        .unwrap_or(home_dir.join("AppData").join("Roaming"))
        .join("echophrase")
        .join("models");

    log_debug("🔍 Checking possible cache directories:".to_string());
    log_debug(format!("  📁 Checking: {:?}", echophrase_models));

    if !echophrase_models.exists() {
        log_debug("    ❌ Directory doesn't exist".to_string());
        log_debug("❌ No whisper model files found in echophrase models directory".to_string());
        return Ok(false);
    }

    if let Ok(entries) = std::fs::read_dir(&echophrase_models) {
        for entry in entries {
            if let Ok(entry) = entry {
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy();
                let entry_path = entry.path();
                log_debug(format!("    📁 Found: {}", name));

                if entry_path.is_file()
                    && name.to_lowercase().contains("whisper")
                    && name.ends_with(".gguf")
                {
                    let file_size = std::fs::metadata(&entry_path).map(|m| m.len()).unwrap_or(0);
                    let size_mb = file_size as f64 / (1024.0 * 1024.0);
                    if size_mb > 50.0 {
                        log_debug(format!(
                            "    ✅ Valid whisper GGUF model found: {:.1} MB",
                            size_mb
                        ));
                        return Ok(true);
                    } else {
                        log_debug(format!(
                            "    ❌ GGUF file too small ({:.1} MB) - likely corrupted, removing",
                            size_mb
                        ));
                        let _ = std::fs::remove_file(&entry_path);
                    }
                }

                if entry_path.is_dir() {
                    if let Some(found) = check_model_in_dir(&entry_path, &name) {
                        return Ok(found);
                    }
                }
            }
        }
    }

    log_debug("❌ No whisper model files found in echophrase models directory".to_string());
    Ok(false)
}

/// Check if a model directory (or its subdirs) contains a complete model file.
/// Returns `Some(true)` if found, `None` to continue searching.
fn check_model_in_dir(entry_path: &std::path::Path, name: &str) -> Option<bool> {
    log_debug(format!("    📂 Exploring directory: {}", name));

    let lower = name.to_lowercase();
    let mut has_whisper_name = lower.contains("whisper") || lower.contains("openai");

    if !has_whisper_name {
        if let Ok(sub_entries) = std::fs::read_dir(entry_path) {
            for sub_entry in sub_entries.flatten() {
                let sub_name = sub_entry.file_name();
                if sub_name
                    .to_string_lossy()
                    .to_lowercase()
                    .contains("whisper")
                {
                    has_whisper_name = true;
                    break;
                }
            }
        }
    }

    if !has_whisper_name {
        return None;
    }

    if has_complete_model_files(entry_path) {
        log_debug(format!("    ✅ Found model files directly in {}", name));
        return Some(true);
    }

    let main_dir = entry_path.join("main");
    if main_dir.exists() && has_complete_model_files(&main_dir) {
        log_debug(format!("    ✅ Found model files in main/ under {}", name));
        return Some(true);
    }

    let snapshots_dir = entry_path.join("snapshots");
    if snapshots_dir.exists() {
        if let Ok(snapshot_entries) = std::fs::read_dir(&snapshots_dir) {
            for snapshot_entry in snapshot_entries.flatten() {
                let snapshot_path = snapshot_entry.path();
                if has_any_model_files(&snapshot_path) {
                    log_debug(format!(
                        "    ✅ Found model files in snapshot under {}",
                        name
                    ));
                    return Some(true);
                }
            }
        }
    }

    None
}

fn has_complete_model_files(dir: &std::path::Path) -> bool {
    let pairs = [
        ("model.bin", "model.bin.partial"),
        ("model.safetensors", "model.safetensors.partial"),
        ("pytorch_model.bin", "pytorch_model.bin.partial"),
        ("model.gguf", "model.gguf.partial"),
    ];
    pairs
        .iter()
        .any(|(file, partial)| dir.join(file).exists() && !dir.join(partial).exists())
}

fn has_any_model_files(dir: &std::path::Path) -> bool {
    [
        "model.bin",
        "model.safetensors",
        "pytorch_model.bin",
        "model.gguf",
    ]
    .iter()
    .any(|f| dir.join(f).exists())
}

/// Checks download status for ALL models in the registry.
/// Uses the unified ModelFileChecker trait for format-agnostic detection.
/// Returns Vec<(model_id, is_downloaded)> for frontend consumption.
#[tauri::command]
pub async fn check_models_exist() -> Result<Vec<(String, bool)>, String> {
    use crate::models::MODEL_REGISTRY;

    log_debug("🔍 Checking all models in registry...".to_string());

    let mut results: Vec<(String, bool)> = MODEL_REGISTRY
        .iter()
        .map(|model| {
            let downloaded = model.is_downloaded();
            if downloaded {
                log_debug(format!(
                    "✅ Model '{}' ({}) is downloaded",
                    model.id, model.name
                ));
            } else {
                log_debug(format!(
                    "❌ Model '{}' ({}) not found",
                    model.id, model.name
                ));
            }
            (model.id.to_string(), downloaded)
        })
        .collect();

    log_debug(format!(
        "📊 Model check complete: {} models checked",
        results.len()
    ));

    // BYO custom model: "downloaded" when an imported model.onnx is present.
    let settings = crate::settings::AppSettings::load().to_cmd()?;
    let custom_imported = settings
        .custom_model_path
        .as_deref()
        .map(|dir| std::path::Path::new(dir).join("model.onnx").exists())
        .unwrap_or(false);
    results.push(("custom".to_string(), custom_imported));

    Ok(results)
}

// ---------------------------------------------------------------------------
// Download commands - shells that delegate to model_download::*
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn cancel_model_download() -> Result<(), String> {
    log_debug("🛑 Cancelling current model download...".to_string());
    DOWNLOAD_CANCELLED.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub async fn switch_model_download(
    app_handle: AppHandle,
    model_type: String,
) -> Result<(), String> {
    log_debug(format!("🔄 Switching to download {} model", model_type));
    DOWNLOAD_CANCELLED.store(true, Ordering::SeqCst);

    // Via SettingsCache so the frontend gets `settings-changed` and resyncs -
    // this path changes the model behind the UI's back.
    let selected = model_type.clone();
    crate::settings::cache::SettingsCache::update(&app_handle, |s| s.model = selected)?;

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    DOWNLOAD_CANCELLED.store(false, Ordering::SeqCst);

    crate::model_download::download_model_with_progress(app_handle, Some(model_type))
        .await
        .to_cmd()
}

#[tauri::command]
pub async fn download_whisper_model(
    app_handle: AppHandle,
    model_type: Option<String>,
) -> Result<(), String> {
    DOWNLOAD_CANCELLED.store(false, Ordering::SeqCst);
    crate::model_download::download_model_with_progress(app_handle, model_type)
        .await
        .map_err(|e| format!("Model download failed: {}", e))
}

// ---------------------------------------------------------------------------
// Audio device commands - thin forwarders to audio_devices module.
// TODO(group-a): update lib.rs handler list to reference
//   commands::audio_devices::check_microphone_available and
//   commands::audio_devices::get_audio_input_devices directly.
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn check_microphone_available() -> Result<bool, String> {
    super::audio_devices::check_microphone_available().await
}

#[tauri::command]
pub async fn get_audio_input_devices() -> Result<Vec<serde_json::Value>, String> {
    super::audio_devices::get_audio_input_devices().await
}

// ---------------------------------------------------------------------------
// Transcription commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn transcribe_audio(app_handle: AppHandle, file_path: String) -> Result<String, String> {
    // Native (macOS/Windows) recording transcribes via this file-based path.
    // Flip the tray to Transcribing for the duration and back to Idle on every
    // exit (Ok, Err, panic-unwind) so the indicator matches the WSL buffer path.
    let _tray_guard = crate::tray_status::StatusGuard::new(
        app_handle.clone(),
        crate::tray_status::TrayStatus::Transcribing,
        crate::tray_status::TrayStatus::Idle,
    );

    log::info!("=== TRANSCRIBE DEBUG ===");
    log::info!("Requested path: {}", file_path);
    log::info!("Current dir: {:?}", std::env::current_dir());
    log::info!("File exists: {}", std::path::Path::new(&file_path).exists());

    match create_transcription(&file_path, app_handle.clone()).await {
        Ok(result) => Ok(result.text),
        Err(e) => Err(format!("Transcription failed: {}", e)),
    }
}

/// Result of a transcription operation.
pub struct TranscriptionResult {
    pub text: String,
}

/// Pure transcription function without Tauri dependencies.
pub async fn transcribe_audio_file(
    file_path: &str,
) -> Result<TranscriptionResult, Box<dyn std::error::Error>> {
    let absolute_path = if std::path::Path::new(&file_path).is_absolute() {
        std::path::PathBuf::from(&file_path)
    } else {
        std::env::current_dir()?.join(&file_path)
    };

    log::info!("Resolved absolute path: {:?}", absolute_path);
    log::info!("Final file exists: {}", absolute_path.exists());

    if !absolute_path.exists() {
        return Err(format!("Audio file not found at: {:?}", absolute_path).into());
    }

    use crate::models::is_onnx_model;
    use std::time::Instant;

    let settings = crate::settings::AppSettings::load()?;
    let model_id = &settings.model;
    let start_time = Instant::now();

    let (backend_name, compute_info) = if model_id == "custom" || is_onnx_model(model_id) {
        let provider = if !crate::is_onnx_runtime_available() {
            "unavailable - ORT not loaded".to_string()
        } else {
            #[cfg(target_os = "macos")]
            {
                "CoreML (Metal)".to_string()
            }
            #[cfg(target_os = "windows")]
            {
                "DirectML".to_string()
            }
            #[cfg(target_os = "linux")]
            {
                "CPU".to_string()
            }
        };
        ("ONNX Runtime", provider)
    } else {
        #[cfg(target_os = "macos")]
        let device = {
            if crate::platform::macos::is_macos_13_or_later() {
                "Metal GPU"
            } else {
                "CPU (macOS < 13)"
            }
        };
        #[cfg(not(target_os = "macos"))]
        let device = "CPU";
        ("Candle", device.to_string())
    };

    log::info!(
        "=== TRANSCRIPTION START === model: {} | backend: {} | compute: {}",
        model_id,
        backend_name,
        compute_info
    );

    // Single dispatch point: the router resolves the file through AudioSource
    // and picks the ONNX/Candle backend from settings.
    let source = echo_audio::AudioSource::from_path(&absolute_path);
    let result = crate::transcription::transcribe_source(&source, None).await;

    match result {
        Ok(transcript) => {
            let elapsed = start_time.elapsed();
            let elapsed_secs = elapsed.as_secs();
            let elapsed_ms = elapsed.as_millis();
            if elapsed_secs > 0 {
                log::info!("✅ Transcription successful ({}s)", elapsed_secs);
            } else {
                log::info!("✅ Transcription successful ({}ms)", elapsed_ms);
            }
            Ok(TranscriptionResult { text: transcript })
        }
        Err(backend_err) => {
            log::error!("❌ Transcription failed: {}", backend_err);
            Err(format!("Transcription failed: {}", backend_err).into())
        }
    }
}

async fn create_transcription(
    file_path: &str,
    _app_handle: AppHandle,
) -> Result<TranscriptionResult, Box<dyn std::error::Error>> {
    transcribe_audio_file(file_path).await
}

// ---------------------------------------------------------------------------
// Model management commands
// ---------------------------------------------------------------------------

/// Delete a downloaded model from the HuggingFace cache.
#[tauri::command]
pub async fn delete_model(model_type: String) -> Result<(), String> {
    log_debug(format!("🗑️ Deleting model: {}", model_type));

    let hf_cache_dir = crate::models::utils::get_hf_cache_dir()?;
    let cache_dir_name = crate::models::get_hf_cache_dir_name(&model_type)
        .ok_or_else(|| format!("Unknown model type: {}", model_type))?;
    let model_dir = hf_cache_dir.join(&cache_dir_name);

    log_debug(format!("📁 Looking for model at: {:?}", model_dir));

    if !model_dir.exists() {
        return Err(format!(
            "Model '{}' not found at {:?}",
            model_type, model_dir
        ));
    }

    match std::fs::remove_dir_all(&model_dir) {
        Ok(_) => {
            log_debug(format!(
                "✅ Successfully deleted model: {} from {:?}",
                model_type, model_dir
            ));
            Ok(())
        }
        Err(e) => {
            log_debug(format!("❌ Failed to delete model {}: {}", model_type, e));
            Err(format!("Failed to delete model: {}", e))
        }
    }
}

/// Returns the directory containing downloaded voice models (the HuggingFace
/// hub cache), creating it if absent so the "open models folder" button in
/// Settings always has a target - `open_directory` errors on a missing path.
#[tauri::command]
pub fn get_models_dir() -> Result<String, String> {
    let hf_cache_dir = crate::models::utils::get_hf_cache_dir()?;
    std::fs::create_dir_all(&hf_cache_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;
    Ok(hf_cache_dir.to_string_lossy().to_string())
}

/// Check if a model has a partial download and return its size.
#[tauri::command]
pub async fn check_partial_download(model_type: String) -> Result<Option<u64>, String> {
    let hf_cache_dir = crate::models::utils::get_hf_cache_dir()?;
    let cache_dir_name = match crate::models::get_hf_cache_dir_name(&model_type) {
        Some(name) => name,
        None => return Ok(None),
    };
    let model_dir = hf_cache_dir.join(&cache_dir_name);
    if !model_dir.exists() {
        return Ok(None);
    }

    let total = crate::model_download::sum_partial_files(&model_dir);
    if total > 0 {
        log_debug(format!(
            "📦 Total partial bytes for {}: {}",
            model_type, total
        ));
        Ok(Some(total))
    } else {
        Ok(None)
    }
}

/// Delete partial download files for a model.
#[tauri::command]
pub async fn delete_partial_download(model_type: String) -> Result<(), String> {
    log_debug(format!("🗑️ Deleting partial download for: {}", model_type));

    let hf_cache_dir = crate::models::utils::get_hf_cache_dir()?;
    let cache_dir_name = crate::models::get_hf_cache_dir_name(&model_type)
        .ok_or_else(|| format!("Unknown model type: {}", model_type))?;
    let model_dir = hf_cache_dir.join(&cache_dir_name);

    if model_dir.exists() {
        std::fs::remove_dir_all(&model_dir)
            .map_err(|e| format!("Failed to delete partial download: {}", e))?;
        log_debug(format!(
            "✅ Deleted partial download directory: {:?}",
            model_dir
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Model preload commands
// ---------------------------------------------------------------------------

/// Preload the model into GPU memory for instant transcriptions.
/// Checks selected model type and only preloads if applicable.
///
/// Emits `model-preload-started` and `model-preload-finished` events so the
/// frontend can observe whether the blocking backend call ever entered.
///
/// Concurrent callers are serialised on `PRELOAD_LOCK`: the second caller waits
/// for the first to finish, then returns immediately if the model it wanted is
/// the one that just loaded. By then `model-preload-finished` has been emitted,
/// so the frontend's `preloadPhase` is already `'done'` when this resolves.
#[tauri::command]
pub async fn preload_whisper_model(app_handle: AppHandle) -> Result<(), String> {
    let _guard = PRELOAD_LOCK.lock().await;

    let settings = crate::settings::AppSettings::load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;
    let model_id = settings.model.clone();

    if LOADED_MODEL.lock().ok().and_then(|m| m.clone()).as_deref() == Some(model_id.as_str()) {
        log_debug(format!(
            "⏭️ Model {} already preloaded - skipping duplicate preload",
            model_id
        ));
        // Still emit the lifecycle events. A caller on a FRESH page (HMR reload,
        // or any re-init after the model was loaded earlier) has a `preloadPhase`
        // of 'idle' and no in-flight events to observe - returning a silent
        // `Ok(())` would leave it stuck there and make it report the preload as
        // incomplete even though the model is resident.
        let _ = app_handle.emit(
            "model-preload-status",
            serde_json::json!({ "phase": "done", "elapsed_ms": 0, "error": None::<String> }),
        );
        let _ = app_handle.emit(
            "model-preload-finished",
            serde_json::json!({
                "backend": "cached", "model": model_id,
                "duration_ms": 0, "ok": true, "error": None::<String>,
            }),
        );
        return Ok(());
    }

    let result = preload_whisper_model_inner(app_handle).await;
    if result.is_ok() {
        if let Ok(mut loaded) = LOADED_MODEL.lock() {
            *loaded = Some(model_id);
        }
    }
    result
}

async fn preload_whisper_model_inner(app_handle: AppHandle) -> Result<(), String> {
    log_debug("🔥 Preloading Whisper model...".to_string());

    let settings = crate::settings::AppSettings::load()
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    let model_id = settings.model.clone();

    // Dispatch order MUST mirror transcription::transcribe_source, or preload
    // loads a different backend than transcribe uses. Qwen3-ASR is registered
    // with backend: Onnx but has its own loader, so it comes BEFORE the generic
    // ONNX branch - previously it fell through to OnnxModelType::Custom and
    // preloaded nothing, making the switch look instant while the first
    // transcription paid a ~10s lazy load with no progress UI.
    if model_id.starts_with("qwen3-asr") {
        if !crate::is_onnx_runtime_available() {
            let msg = "ONNX Runtime is not available on this system - skipping Qwen3-ASR preload";
            log_debug(msg.to_string());
            return Err(msg.to_string());
        }
        let id = model_id.clone();
        return preload_with_events(&app_handle, "qwen3-asr", &model_id, move || {
            crate::transcription::qwen3_asr::preload(&id)
        })
        .await;
    }

    // "custom" is a BYO sentinel, not a MODEL_REGISTRY entry, so
    // is_onnx_model's registry lookup never matches it even though BYO
    // import only accepts .onnx files - must mirror transcribe_source's
    // same special case or preload silently loads Candle/Whisper instead.
    if model_id == "custom" || crate::models::is_onnx_model(&model_id) {
        if !crate::is_onnx_runtime_available() {
            let msg = "ONNX Runtime is not available on this system - skipping ONNX model preload";
            log_debug(msg.to_string());
            return Err(msg.to_string());
        }
        let id = model_id.clone();
        let app_for_task = app_handle.clone();
        return preload_with_events(&app_handle, "onnx", &model_id, move || {
            crate::transcription::onnx_backend::preload_onnx_model(&id, &app_for_task)
        })
        .await;
    }

    preload_with_events(&app_handle, "candle", &model_id, || {
        crate::transcription::candle_whisper::preload_model()
    })
    .await
}

/// Run a backend's blocking load with the shared preload lifecycle:
/// `model-preload-started` -> timeout-bounded `spawn_blocking` ->
/// `model-preload-status` + `model-preload-finished`.
///
/// Every backend MUST preload through here. Each one used to hand-roll this
/// sequence, which is how Qwen3-ASR ended up loading with no progress events
/// at all - the UI showed a loading bar for ~1ms while the real load happened
/// later, lazily, inside transcribe.
///
/// `load` is the backend's own blocking loader (see `TranscriptionBackend`
/// implementations); it runs on a blocking thread and must be self-contained.
async fn preload_with_events<F>(
    app_handle: &AppHandle,
    backend: &str,
    model_id: &str,
    load: F,
) -> Result<(), String>
where
    F: FnOnce() -> anyhow::Result<()> + Send + 'static,
{
    log_debug(format!("Preloading {} model: {}", backend, model_id));
    let started = std::time::Instant::now();
    let _ = app_handle.emit(
        "model-preload-started",
        serde_json::json!({ "backend": backend, "model": model_id }),
    );

    let task_result = tokio::time::timeout(
        std::time::Duration::from_secs(PRELOAD_CEILING_SECS),
        tokio::task::spawn_blocking(load),
    )
    .await;

    let duration_ms = started.elapsed().as_millis() as u64;

    let outcome: Result<(), String> = match task_result {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(e))) => Err(e.to_string()),
        Ok(Err(join_err)) => Err(format!("Task join error: {}", join_err)),
        Err(_timeout) => Err(format!(
            "{} preload timed out after {}s",
            backend, PRELOAD_CEILING_SECS
        )),
    };

    let ok = outcome.is_ok();
    let error = outcome.as_ref().err().cloned();

    let _ = app_handle.emit(
        "model-preload-status",
        serde_json::json!({
            "phase": if ok { "done" } else { "failed" },
            "elapsed_ms": duration_ms,
            "error": error,
        }),
    );
    let _ = app_handle.emit(
        "model-preload-finished",
        serde_json::json!({
            "backend": backend, "model": model_id,
            "duration_ms": duration_ms, "ok": ok, "error": error,
        }),
    );

    match &outcome {
        Ok(()) => log_debug(format!(
            "{} preloaded successfully in {}ms",
            backend, duration_ms
        )),
        Err(e) => log_debug(format!("{} preload failed: {}", backend, e)),
    }
    outcome
}

/// Clear the Whisper model cache (call before switching models).
#[tauri::command]
pub fn clear_whisper_model_cache() {
    log_debug("🗑️ Clearing Whisper model cache...".to_string());
    crate::transcription::candle_whisper::clear_model_cache();
    // This evicts Candle models, so the LOADED_MODEL record would otherwise go
    // stale: the next preload would take the already-loaded fast path, load
    // nothing, and the first transcribe would pay a silent cold load. Cleared
    // unconditionally - a redundant preload is cheap (each backend's own cache
    // short-circuits) whereas a stale "it's loaded" claim is a silent stall.
    if let Ok(mut loaded) = LOADED_MODEL.lock() {
        *loaded = None;
    }
    log_debug("✅ Whisper model cache cleared".to_string());
}

/// Unload the currently-loaded transcription model from memory.
///
/// Clears both the Candle (Whisper) and ONNX (Parakeet) caches and clears
/// `LOADED_MODEL` so the next `preload_whisper_model` call actually does work
/// (rather than early-returning on the already-preloaded fast path).
///
/// Used by the "Reload model" debug button to profile cold-load times.
#[tauri::command]
pub fn unload_model() {
    log_debug("🗑️ Unloading all model caches (Candle + ONNX + Qwen3)...".to_string());
    crate::transcription::candle_whisper::clear_model_cache();
    crate::transcription::onnx_backend::clear_onnx_cache();
    // Qwen3-ASR keeps its own cache (CACHED_QWEN3), so clearing the generic
    // ONNX cache does not touch it - without this it stayed resident (~2GB)
    // after switching to another model.
    crate::transcription::qwen3_asr::Qwen3AsrBackend::unload();
    // Nothing is resident any more, so the next preload must be a real cold
    // load rather than hitting the already-loaded fast path.
    if let Ok(mut loaded) = LOADED_MODEL.lock() {
        *loaded = None;
    }
    log_debug("✅ All model caches unloaded - next preload will be a cold start".to_string());
}

#[cfg(test)]
mod macos_audio_tests {
    // Every test in here is #[cfg(target_os = "macos")], so on other platforms
    // the module compiles empty and a plain `use super::*` is an unused import.
    #[cfg(target_os = "macos")]
    use super::*;

    // check_microphone_available: macOS path uses system_profiler and returns Ok(bool)
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn test_check_microphone_available_returns_ok_on_macos() {
        let result = check_microphone_available().await;
        assert!(
            result.is_ok(),
            "check_microphone_available should not error on macOS"
        );
    }

    // get_audio_input_devices: cpal path should return Ok on macOS (empty vec is fine)
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn test_get_audio_input_devices_returns_ok_on_macos() {
        let result = get_audio_input_devices().await;
        assert!(
            result.is_ok(),
            "get_audio_input_devices should not error on macOS"
        );
        // Each device entry must have "name" and "id" keys
        if let Ok(devices) = result {
            for device in &devices {
                assert!(
                    device.get("name").is_some(),
                    "device entry missing 'name' key"
                );
                assert!(device.get("id").is_some(), "device entry missing 'id' key");
            }
        }
    }

    // On macOS with no input devices (Mac mini), the list should be empty
    // and check_microphone_available should return false
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn test_no_input_devices_does_not_panic() {
        // This test verifies the functions handle zero-device scenarios gracefully.
        // On a Mac mini with no connected mic both return Ok without panicking.
        let mic_check = check_microphone_available().await;
        let device_list = get_audio_input_devices().await;
        assert!(mic_check.is_ok());
        assert!(device_list.is_ok());
        // Consistency check: if device list is empty, mic should not be available
        if let (Ok(false), Ok(devices)) = (&mic_check, &device_list) {
            assert!(
                devices.is_empty(),
                "mic reported unavailable but devices list is non-empty: {:?}",
                devices
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires ONNX Runtime - run with: cargo test -- --ignored"]
    async fn test_transcribe_audio_file() {
        use crate::transcription::execution_provider::{
            get_execution_provider_status, get_platform_default,
        };
        use std::time::Instant;

        let test_path = "../tests/audio/test-1-2.wav";

        log::info!("🧪 Testing pure transcription function...");
        log::info!("📁 Test file: {}", test_path);

        let ep = get_platform_default();
        let status = get_execution_provider_status(None);
        log::info!("\n🔧 Execution Provider: {}", ep);
        log::info!("   Active: {}", status.active);
        log::info!("   Status: {}", status.status_message);
        log::info!("   CPU Fallback: {}", status.is_cpu_fallback);

        #[cfg(windows)]
        {
            use crate::transcription::execution_provider::{
                ExecutionProviderType, verify_directml_available,
            };
            assert_eq!(
                status.active,
                ExecutionProviderType::DirectMl,
                "Windows should use DirectML, got: {}",
                status.active
            );
            assert!(
                !status.is_cpu_fallback,
                "DirectML should not fall back to CPU"
            );
            let dml_result = verify_directml_available();
            assert!(
                dml_result.is_ok(),
                "❌ DirectML verification FAILED: {}",
                dml_result.err().unwrap_or_default()
            );
            log::info!("   DirectML verified: EP can be registered");
        }

        #[cfg(target_os = "macos")]
        {
            use crate::transcription::execution_provider::ExecutionProviderType;
            assert_eq!(
                status.active,
                ExecutionProviderType::CoreMl,
                "macOS should use CoreML, got: {}",
                status.active
            );
            log::info!("   (macOS: CoreML expected - confirmed)");
        }

        // Linux (and other non-macOS Unix): CPU only.
        #[cfg(all(not(windows), not(target_os = "macos")))]
        {
            use crate::transcription::execution_provider::ExecutionProviderType;
            assert_eq!(
                status.active,
                ExecutionProviderType::Cpu,
                "Linux should use CPU, got: {}",
                status.active
            );
            log::info!("   (Linux: CPU expected - confirmed)");
        }

        log::info!("\n⏳ Warmup run (includes model loading + shader compilation)...");
        let warmup_start = Instant::now();
        let warmup_result = transcribe_audio_file(test_path).await;
        let warmup_time = warmup_start.elapsed();
        log::info!("⏱️  Warmup time: {:.2}s", warmup_time.as_secs_f64());

        if let Err(e) = &warmup_result {
            panic!("❌ Warmup transcription failed: {}", e);
        }

        log::info!("\n🚀 Benchmark runs (model cached, pure inference)...");
        let mut times = Vec::new();
        for i in 1..=5 {
            let bench_start = Instant::now();
            let result = transcribe_audio_file(test_path).await;
            let bench_time = bench_start.elapsed();
            times.push(bench_time.as_secs_f64());
            match &result {
                Ok(r) => log::info!(
                    "   Run {}: {:.3}s - '{}'",
                    i,
                    bench_time.as_secs_f64(),
                    r.text
                ),
                Err(e) => panic!("❌ Transcription failed on run {}: {}", i, e),
            }
        }

        let avg_time = times.iter().sum::<f64>() / times.len() as f64;
        let min_time = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_time = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        log::info!("\n📊 BENCHMARK RESULTS (5 runs):");
        log::info!(
            "   Min: {:.3}s | Max: {:.3}s | Avg: {:.3}s",
            min_time,
            max_time,
            avg_time
        );

        #[cfg(windows)]
        {
            assert!(
                avg_time < 0.25,
                "❌ FAIL: DirectML inference too slow ({:.3}s avg) - likely CPU fallback! Expected <0.25s",
                avg_time
            );
            log::info!(
                "✅ DirectML GPU verified: {:.3}s avg (threshold: <0.25s)",
                avg_time
            );
        }

        #[cfg(not(windows))]
        {
            log::info!("✅ CPU inference: {:.3}s avg", avg_time);
        }
    }
}
