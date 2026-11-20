// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use futures_util::StreamExt;
use serde_json::json;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::time::Instant;

use crate::commands::transcription::{DOWNLOAD_CANCELLED, DownloadStatus, log_debug};
use crate::model_download::progress::format_time;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Download the model specified by `model_type` (or the one selected in
/// settings when `None`), streaming progress events to the frontend.
pub async fn download_model_with_progress(
    app_handle: AppHandle,
    model_type: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Reject an overlapping download (registry or BYO custom-model import -
    // see DOWNLOAD_SESSION_LOCK) instead of corrupting shared progress state.
    let _guard = crate::commands::transcription::acquire_download_lock().await?;

    let model_id = resolve_model_id(&model_type)?;

    // Registry path (new multi-file models)
    if let Some(registry_entry) = crate::models::get_model_info(&model_id) {
        download_registry_model(app_handle, &model_id, &registry_entry).await?;
        return Ok(());
    }

    // Legacy single-file path
    download_legacy_model(app_handle, model_type).await
}

// ---------------------------------------------------------------------------
// Registry-based download (new models)
// ---------------------------------------------------------------------------

async fn download_registry_model(
    app_handle: AppHandle,
    model_id: &str,
    registry_entry: &crate::models::ModelRegistryEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    log_debug(format!(
        "📥 Downloading {} ({} MB)",
        registry_entry.name, registry_entry.expected_size_mb
    ));

    emit_download_start(&app_handle, model_id, registry_entry);

    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": 0,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": "0s",
            "status": "Preparing download..."
        }),
    );

    let start_time = Instant::now();
    let model_dir = build_hf_model_dir(&registry_entry.repo_id)?;
    std::fs::create_dir_all(&model_dir)?;

    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": 0,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": "0s",
            "status": "Checking existing files..."
        }),
    );

    let client = reqwest::Client::new();
    let total_files = registry_entry.model_files.len();
    let total_expected_bytes = registry_entry.expected_size_mb * 1_000_000;
    let mut cumulative_downloaded: u64 = 0;

    for (i, file_path) in registry_entry.model_files.iter().enumerate() {
        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            registry_entry.repo_id, file_path
        );
        let final_path = model_dir.join(file_path);

        let freshly_downloaded = download_single_file(
            &client,
            &url,
            &final_path,
            &app_handle,
            i,
            total_files,
            &mut cumulative_downloaded,
            total_expected_bytes,
        )
        .await?;

        if DOWNLOAD_CANCELLED.load(Ordering::SeqCst) {
            let _ = app_handle.emit("model-download-cancelled", ());
            return Ok(());
        }

        // Integrity gate: freshly downloaded binaries must match their pinned
        // SHA256 (see manifest.rs). Files already on disk from prior sessions
        // were verified when first downloaded and re-hashing multi-GB files on
        // every check would be prohibitively slow.
        if freshly_downloaded {
            let _ = app_handle.emit("model-download-progress", json!({
                "progress": ((cumulative_downloaded as f64 / total_expected_bytes.max(1) as f64) * 100.0).min(99.0) as u32,
                "speed": "",
                "timeRemaining": "",
                "timeElapsed": format_time(start_time.elapsed().as_secs()),
                "status": DownloadStatus::VerifyingIntegrity
            }));
            crate::model_download::manifest::verify_pinned_file(
                registry_entry.repo_id,
                file_path,
                &final_path,
            )
            .await?;
        }
    }

    // Config files for Candle models
    if registry_entry.backend == crate::models::ModelBackend::Candle {
        download_candle_config_files(&client, &registry_entry.repo_id, &model_dir).await;
    }

    emit_download_complete(&app_handle, model_id, start_time.elapsed().as_secs());
    log_debug(format!(
        "🎉 {} downloaded successfully!",
        registry_entry.name
    ));
    Ok(())
}

fn emit_download_start(
    app_handle: &AppHandle,
    model_id: &str,
    registry_entry: &crate::models::ModelRegistryEntry,
) {
    let _ = app_handle.emit(
        "model-download-start",
        serde_json::json!({
            "model_id": model_id,
            "display_name": registry_entry.name,
            "expected_size_mb": registry_entry.expected_size_mb,
        }),
    );
}

fn build_hf_model_dir(repo_id: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let home_dir = dirs::home_dir().ok_or("Home directory not found")?;
    Ok(home_dir
        .join(".cache")
        .join("huggingface")
        .join("hub")
        .join(format!("models--{}", repo_id.replace("/", "--")))
        .join("snapshots")
        .join("main"))
}

async fn download_candle_config_files(
    client: &reqwest::Client,
    repo_id: &str,
    model_dir: &std::path::Path,
) {
    let config_files = ["config.json", "tokenizer.json", "preprocessor_config.json"];
    for config_file in &config_files {
        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_id, config_file
        );
        let final_path = model_dir.join(config_file);
        if !final_path.exists() {
            log_debug(format!("📥 Downloading config: {}", config_file));
            if let Ok(response) = client.get(&url).send().await {
                if response.status().is_success() {
                    if let Ok(bytes) = response.bytes().await {
                        let _ = tokio::fs::write(&final_path, bytes).await;
                    }
                }
            }
        }
    }
}

fn emit_download_complete(app_handle: &AppHandle, model_id: &str, elapsed_secs: u64) {
    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": 100,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": format_time(elapsed_secs),
            "status": DownloadStatus::Complete
        }),
    );
    let _ = app_handle.emit("model-download-complete", model_id);
    let _ = app_handle.emit("model-now-exists", true);
}

// ---------------------------------------------------------------------------
// Single file streaming download (multi-file registry)
// ---------------------------------------------------------------------------

/// Downloads a single file from HuggingFace with progress tracking.
///
/// Uses cumulative byte tracking across all files for smooth, proportional
/// progress. `cumulative_downloaded` is updated in-place so the caller can
/// pass it to subsequent files.
///
/// Returns `true` when the file was freshly downloaded, `false` when it
/// already existed on disk and was skipped (callers use this to decide
/// whether to run integrity verification).
pub async fn download_single_file(
    client: &reqwest::Client,
    url: &str,
    final_path: &std::path::Path,
    app_handle: &AppHandle,
    file_index: usize,
    total_files: usize,
    cumulative_downloaded: &mut u64,
    total_expected_bytes: u64,
) -> Result<bool, Box<dyn std::error::Error>> {
    let filename = final_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let partial_path = final_path.with_extension(format!(
        "{}.partial",
        final_path.extension().unwrap_or_default().to_string_lossy()
    ));

    // Skip already-downloaded file
    if enumerate_existing_file(final_path, &filename, cumulative_downloaded)? {
        return Ok(false);
    }

    log_debug(format!(
        "📥 Downloading file {}/{}: {}",
        file_index + 1,
        total_files,
        filename
    ));

    let initial_size = partial_size(&partial_path);

    emit_connecting_progress(
        app_handle,
        &filename,
        file_index,
        total_files,
        initial_size,
        *cumulative_downloaded,
        total_expected_bytes,
    );

    let (response, _initial_cumulative_offset) =
        build_and_send_request(client, url, initial_size, cumulative_downloaded).await?;

    let partial_path_clone = partial_path.clone();
    let mut file = open_partial_file(
        &partial_path_clone,
        initial_size,
        response.status().as_u16(),
    )
    .await?;

    stream_to_disk(
        response,
        &mut file,
        &filename,
        file_index,
        total_files,
        initial_size,
        cumulative_downloaded,
        total_expected_bytes,
        app_handle,
    )
    .await?;

    file.flush().await?;
    drop(file);
    std::fs::rename(&partial_path, final_path)?;

    let final_size = std::fs::metadata(final_path).map(|m| m.len()).unwrap_or(0);
    log_debug(format!(
        "  ✓ {} downloaded ({:.1} MB)",
        filename,
        final_size as f64 / (1024.0 * 1024.0)
    ));

    Ok(true)
}

/// Returns `true` and updates cumulative bytes if the file already exists.
fn enumerate_existing_file(
    final_path: &std::path::Path,
    filename: &str,
    cumulative_downloaded: &mut u64,
) -> Result<bool, Box<dyn std::error::Error>> {
    if final_path.exists() {
        let size = std::fs::metadata(final_path).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            log_debug(format!(
                "  ✓ {} already exists ({:.1} MB)",
                filename,
                size as f64 / (1024.0 * 1024.0)
            ));
            *cumulative_downloaded += size;
            return Ok(true);
        }
    }

    // Ensure parent directory exists
    if let Some(parent) = final_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    Ok(false)
}

fn partial_size(partial_path: &std::path::Path) -> u64 {
    if partial_path.exists() {
        std::fs::metadata(partial_path)
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    }
}

fn emit_connecting_progress(
    app_handle: &AppHandle,
    filename: &str,
    file_index: usize,
    total_files: usize,
    initial_size: u64,
    cumulative_downloaded: u64,
    total_expected_bytes: u64,
) {
    let current_progress = if total_expected_bytes > 0 {
        ((cumulative_downloaded as f64 / total_expected_bytes as f64) * 100.0).min(99.0) as u32
    } else {
        0
    };
    let status_msg = if initial_size > 0 {
        format!(
            "Resuming {} ({:.0} MB)...",
            filename,
            initial_size as f64 / (1024.0 * 1024.0)
        )
    } else if total_files > 1 {
        format!("Connecting ({}/{})...", file_index + 1, total_files)
    } else {
        "Connecting to server...".to_string()
    };
    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": current_progress,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": "0s",
            "status": status_msg
        }),
    );
}

async fn build_and_send_request(
    client: &reqwest::Client,
    url: &str,
    initial_size: u64,
    cumulative_downloaded: &mut u64,
) -> Result<(reqwest::Response, u64), Box<dyn std::error::Error>> {
    let mut request = client.get(url);
    if initial_size > 0 {
        request = request.header("Range", format!("bytes={}-", initial_size));
        log_debug(format!(
            "  📁 Resuming from {:.1} MB",
            initial_size as f64 / (1024.0 * 1024.0)
        ));
    }

    let response = request.send().await?;
    let status = response.status();

    if !status.is_success() && status.as_u16() != 206 {
        return Err(format!("HTTP error {} for {}", status, url).into());
    }

    // Include resumed bytes in cumulative total
    let resumed_bytes = if status.as_u16() == 206 {
        initial_size
    } else {
        0
    };
    *cumulative_downloaded += resumed_bytes;

    Ok((response, resumed_bytes))
}

async fn open_partial_file(
    partial_path: &std::path::Path,
    initial_size: u64,
    response_status: u16,
) -> Result<tokio::fs::File, Box<dyn std::error::Error>> {
    let file = if initial_size > 0 && response_status == 206 {
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(partial_path)
            .await?
    } else {
        tokio::fs::File::create(partial_path).await?
    };
    Ok(file)
}

/// Stream HTTP response body to disk, emitting progress events every 100 ms.
#[allow(clippy::too_many_arguments)]
#[allow(unused_assignments)] // set in-loop, read by emit_file_progress each iteration
async fn stream_to_disk(
    response: reqwest::Response,
    file: &mut tokio::fs::File,
    filename: &str,
    file_index: usize,
    total_files: usize,
    _initial_size: u64,
    cumulative_downloaded: &mut u64,
    total_expected_bytes: u64,
    app_handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = response.bytes_stream();
    let start_time = Instant::now();
    let mut last_progress_time = start_time;
    let mut last_downloaded = *cumulative_downloaded;
    let mut speed_samples: Vec<f64> = Vec::new();
    let mut last_speed_display = String::new();
    let mut last_eta_display = String::new();
    let mut _file_downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        if DOWNLOAD_CANCELLED.load(Ordering::SeqCst) {
            return Err("Download cancelled".into());
        }

        let chunk = chunk?;
        file.write_all(&chunk).await?;
        let chunk_len = chunk.len() as u64;
        _file_downloaded += chunk_len;
        *cumulative_downloaded += chunk_len;

        // Throttle progress events to every 100 ms
        let now = Instant::now();
        if now.duration_since(last_progress_time).as_millis() >= 100 {
            let time_diff = now.duration_since(last_progress_time).as_secs_f64();
            let bytes_diff = cumulative_downloaded.saturating_sub(last_downloaded);
            let instant_speed_mbps = if time_diff > 0.0 {
                (bytes_diff as f64 / (1024.0 * 1024.0)) / time_diff
            } else {
                0.0
            };
            // Rolling average over the last ~10 samples smooths out per-chunk
            // jitter, matching the legacy single-file downloader's approach.
            speed_samples.push(instant_speed_mbps);
            if speed_samples.len() > 10 {
                speed_samples.remove(0);
            }
            let avg_speed_mbps = speed_samples.iter().sum::<f64>() / speed_samples.len() as f64;

            last_speed_display = if avg_speed_mbps > 0.1 {
                format!("{:.1} MB/s", avg_speed_mbps)
            } else {
                String::new()
            };

            let remaining_bytes = total_expected_bytes.saturating_sub(*cumulative_downloaded);
            last_eta_display = if avg_speed_mbps > 0.1 {
                let eta_seconds =
                    ((remaining_bytes as f64 / (1024.0 * 1024.0)) / avg_speed_mbps) as u64;
                if eta_seconds > 60 {
                    format!("{}m", (eta_seconds / 60).max(1))
                } else if eta_seconds > 0 {
                    format!("{}s", eta_seconds)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            emit_file_progress(
                app_handle,
                filename,
                file_index,
                total_files,
                *cumulative_downloaded,
                total_expected_bytes,
                start_time,
                &last_speed_display,
                &last_eta_display,
            );
            last_progress_time = now;
            last_downloaded = *cumulative_downloaded;
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_file_progress(
    app_handle: &AppHandle,
    filename: &str,
    file_index: usize,
    total_files: usize,
    cumulative_downloaded: u64,
    total_expected_bytes: u64,
    start_time: Instant,
    speed: &str,
    time_remaining: &str,
) {
    let overall_progress = if total_expected_bytes > 0 {
        ((cumulative_downloaded as f64 / total_expected_bytes as f64) * 100.0).min(99.0) as u32
    } else {
        0
    };
    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": overall_progress,
            "speed": speed,
            "timeRemaining": time_remaining,
            "timeElapsed": format_time(start_time.elapsed().as_secs()),
            "status": DownloadStatus::Downloading,
            "currentFile": filename,
            "fileIndex": file_index + 1,
            "totalFiles": total_files,
        }),
    );
}

// ---------------------------------------------------------------------------
// Legacy single-file download path (older models not in registry)
// ---------------------------------------------------------------------------

async fn download_legacy_model(
    app_handle: AppHandle,
    model_type: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let model_info = resolve_legacy_model_info(&model_type)?;

    log_debug(format!(
        "📥 Downloading {} ({} MB) - {}",
        model_info.display_name, model_info.expected_size_mb, model_info.description
    ));

    let _ = app_handle.emit("model-download-start", &model_info);
    log_debug("🚀 Using reqwest streaming for real-time progress!".to_string());

    let start_time = Instant::now();
    let model_dir = legacy_model_dir(&model_info)?;
    std::fs::create_dir_all(&model_dir)?;

    let (model_url, final_model_path, partial_model_path) =
        build_legacy_paths(&model_info, &model_dir);

    let initial_size = legacy_partial_size(&partial_model_path);
    emit_legacy_start_progress(&app_handle, initial_size);

    let client = reqwest::Client::new();
    let (response, total_length) =
        legacy_send_request(&client, &model_url, initial_size, &partial_model_path).await?;

    let _downloaded = legacy_stream_to_disk(
        response,
        &partial_model_path,
        initial_size,
        total_length,
        &app_handle,
        &start_time,
    )
    .await?;

    std::fs::rename(&partial_model_path, &final_model_path)?;
    log_debug(format!(
        "📁 Renamed partial file to final: {:?}",
        final_model_path
    ));

    verify_and_finalize_legacy(
        &app_handle,
        &final_model_path,
        &model_dir,
        &model_type,
        &model_info,
        &client,
        start_time,
    )
    .await
}

fn resolve_model_id(model_type: &Option<String>) -> Result<String, Box<dyn std::error::Error>> {
    use crate::error::ToCmdErr;

    if let Some(id) = model_type {
        Ok(id.clone())
    } else {
        let settings = crate::settings::AppSettings::load().to_cmd()?;
        Ok(settings.model.clone())
    }
}

fn resolve_legacy_model_info(
    model_type: &Option<String>,
) -> Result<crate::models::WhisperModelInfo, Box<dyn std::error::Error>> {
    use crate::error::ToCmdErr;

    let info = if let Some(model_type_str) = model_type {
        // "custom" no longer routes through the download pipeline: BYO
        // imports are handled by `commands::custom_model` (local file copy or
        // HF import with validation). The old GGUF-era branch here is gone.
        if model_type_str == "custom" {
            return Err(
                "Custom models are imported via Settings > Custom Model, not downloaded".into(),
            );
        }
        crate::models::get_legacy_model_info(model_type_str)
            .ok_or_else(|| format!("Unknown model: {}", model_type_str))?
    } else {
        let settings = crate::settings::AppSettings::load().to_cmd()?;
        crate::models::get_legacy_model_info(&settings.model)
            .ok_or_else(|| format!("Unknown model in settings: {}", settings.model))?
    };
    Ok(info)
}

fn legacy_model_dir(
    model_info: &crate::models::WhisperModelInfo,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let home_dir = dirs::home_dir().ok_or("Home directory not found")?;
    let cache_dir = home_dir.join(".cache").join("huggingface").join("hub");
    Ok(cache_dir
        .join(format!(
            "models--{}",
            model_info.model_name.replace("/", "--")
        ))
        .join("snapshots")
        .join("main"))
}

fn build_legacy_paths(
    model_info: &crate::models::WhisperModelInfo,
    model_dir: &std::path::Path,
) -> (String, std::path::PathBuf, std::path::PathBuf) {
    let model_filename = &model_info.model_filename;

    let download_base = if model_info.download_url.contains("/resolve/main") {
        model_info.download_url.clone()
    } else if model_info.download_url.contains("/tree/main") {
        model_info
            .download_url
            .replace("/tree/main/", "/resolve/main/")
            .replace("/tree/main", "/resolve/main")
    } else {
        format!(
            "{}/resolve/main",
            model_info.download_url.trim_end_matches('/')
        )
    };
    let model_url = format!("{}/{}", download_base, model_filename);

    let final_model_path = model_dir.join(model_filename);
    let partial_model_path = model_dir.join(format!("{}.partial", model_filename));
    (model_url, final_model_path, partial_model_path)
}

fn legacy_partial_size(partial_model_path: &std::path::Path) -> u64 {
    if partial_model_path.exists() {
        let size = std::fs::metadata(partial_model_path)
            .map(|m| m.len())
            .unwrap_or(0);
        log_debug(format!(
            "📁 Found partial download: {:.1} MB, resuming...",
            size as f64 / (1024.0 * 1024.0)
        ));
        size
    } else {
        0
    }
}

fn emit_legacy_start_progress(app_handle: &AppHandle, initial_size: u64) {
    let status = if initial_size > 0 {
        format!(
            "Resuming from {:.0} MB...",
            initial_size as f64 / (1024.0 * 1024.0)
        )
    } else {
        "Connecting to server...".to_string()
    };
    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": 0,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": "0s",
            "status": status
        }),
    );
}

async fn legacy_send_request(
    client: &reqwest::Client,
    model_url: &str,
    initial_size: u64,
    partial_model_path: &std::path::Path,
) -> Result<(reqwest::Response, u64), Box<dyn std::error::Error>> {
    let mut request = client.get(model_url);
    if initial_size > 0 {
        request = request.header("Range", format!("bytes={}-", initial_size));
    }

    let response = request.send().await?;
    let response_status = response.status();
    let content_length = response.content_length().unwrap_or(0);

    let total_length = if initial_size > 0 {
        if response_status == 206 {
            log_debug("📡 Resume: Server returned 206 Partial Content".to_string());
            initial_size + content_length
        } else {
            log_debug("⚠️ Server doesn't support ranges, restarting download".to_string());
            let _ = std::fs::remove_file(partial_model_path);
            content_length
        }
    } else {
        content_length
    };

    log_debug(format!(
        "📊 Total expected: {} bytes ({:.1} MB)",
        total_length,
        total_length as f64 / (1024.0 * 1024.0)
    ));

    Ok((response, total_length))
}

async fn legacy_stream_to_disk(
    response: reqwest::Response,
    partial_model_path: &std::path::Path,
    initial_size: u64,
    total_length: u64,
    app_handle: &AppHandle,
    start_time: &Instant,
) -> Result<u64, Box<dyn std::error::Error>> {
    let response_status = response.status();
    let mut file = if initial_size > 0 && response_status == 206 {
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(partial_model_path)
            .await?
    } else {
        tokio::fs::File::create(partial_model_path).await?
    };

    let mut stream = response.bytes_stream();
    let mut downloaded = if response_status == 206 {
        initial_size
    } else {
        0
    };
    let mut last_progress_time = *start_time;
    let mut last_downloaded = downloaded;
    let mut last_stats_update = *start_time;
    let mut speed_samples: Vec<f64> = Vec::new();
    let mut last_speed_display = String::new();
    let mut last_eta_display = String::new();

    let initial_progress = if total_length > 0 {
        ((downloaded as f64 / total_length as f64) * 100.0).min(99.0) as u32
    } else {
        0
    };
    let initial_status = if initial_progress > 0 {
        "Downloading"
    } else {
        "Starting download..."
    };
    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": initial_progress,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": "0s",
            "status": initial_status
        }),
    );

    while let Some(chunk) = stream.next().await {
        if DOWNLOAD_CANCELLED.load(Ordering::SeqCst) {
            log_debug("🛑 Download cancelled by user".to_string());
            let _ = app_handle.emit("model-download-cancelled", ());
            return Ok(downloaded);
        }

        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        let now = Instant::now();
        if now.duration_since(last_progress_time).as_millis() >= 100
            && (downloaded - last_downloaded >= 1_000_000
                || now.duration_since(last_progress_time).as_millis() >= 1000)
        {
            let progress = if total_length > 0 {
                ((downloaded as f64 / total_length as f64) * 100.0).min(99.0) as u32
            } else {
                0
            };

            let time_diff = now.duration_since(last_progress_time).as_secs_f64();
            let bytes_diff = downloaded - last_downloaded;
            let instant_speed = if time_diff > 0.0 {
                (bytes_diff as f64) / (1024.0 * 1024.0) / time_diff
            } else {
                0.0
            };

            speed_samples.push(instant_speed);
            if speed_samples.len() > 10 {
                speed_samples.remove(0);
            }

            if now.duration_since(last_stats_update).as_secs() >= 5 {
                let avg_speed = speed_samples.iter().sum::<f64>() / speed_samples.len() as f64;
                let remaining_bytes = total_length.saturating_sub(downloaded);
                let eta_seconds = if avg_speed > 0.1 {
                    (remaining_bytes as f64 / (1024.0 * 1024.0)) / avg_speed
                } else {
                    0.0
                } as u64;

                last_speed_display = if avg_speed > 0.1 {
                    format!("{:.1} MB/s", avg_speed)
                } else {
                    String::new()
                };
                last_eta_display = if eta_seconds > 60 {
                    format!("{}m", (eta_seconds / 60).max(1))
                } else if eta_seconds > 0 {
                    format!("{}s", eta_seconds)
                } else {
                    String::new()
                };
                last_stats_update = now;
            }

            let status = if progress >= 99 {
                DownloadStatus::VerifyingIntegrity
            } else {
                DownloadStatus::Downloading
            };

            let _ = app_handle.emit(
                "model-download-progress",
                json!({
                    "progress": progress,
                    "speed": last_speed_display,
                    "timeRemaining": last_eta_display,
                    "timeElapsed": format_time(start_time.elapsed().as_secs()),
                    "status": status
                }),
            );

            last_progress_time = now;
            last_downloaded = downloaded;
        }
    }

    file.flush().await?;
    Ok(downloaded)
}

#[allow(clippy::too_many_arguments)]
async fn verify_and_finalize_legacy(
    app_handle: &AppHandle,
    final_model_path: &std::path::Path,
    model_dir: &std::path::Path,
    model_type: &Option<String>,
    model_info: &crate::models::WhisperModelInfo,
    client: &reqwest::Client,
    start_time: Instant,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": 99,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": format_time(start_time.elapsed().as_secs()),
            "status": DownloadStatus::VerifyingIntegrity
        }),
    );

    if !final_model_path.exists() {
        let msg = "Download completed but file not found";
        let _ = app_handle.emit("model-download-error", msg);
        return Err(msg.into());
    }

    let file_size = std::fs::metadata(final_model_path)?.len();
    let size_mb = file_size as f64 / (1024.0 * 1024.0);
    log_debug(format!("📊 Downloaded: {:.1} MB", size_mb));

    if size_mb <= 20.0 {
        let error_msg = format!("Downloaded file too small: {:.1} MB", size_mb);
        let _ = app_handle.emit("model-download-error", &error_msg);
        return Err(error_msg.into());
    }

    log_debug("✅ File download completed successfully".to_string());

    // Download companion config files
    download_legacy_config_files(client, model_info, model_dir).await?;

    let model_id = resolve_legacy_id(model_type, model_info);
    let elapsed = start_time.elapsed().as_secs();
    let _ = app_handle.emit(
        "model-download-progress",
        json!({
            "progress": 100,
            "speed": "",
            "timeRemaining": "",
            "timeElapsed": format_time(elapsed),
            "status": DownloadStatus::Complete
        }),
    );
    let _ = app_handle.emit("model-download-complete", &model_id);
    let _ = app_handle.emit("model-now-exists", true);

    log_debug("🎉 CT2 model fully downloaded and ready!".to_string());
    Ok(())
}

async fn download_legacy_config_files(
    client: &reqwest::Client,
    model_info: &crate::models::WhisperModelInfo,
    model_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let download_base = if model_info.download_url.contains("/resolve/main") {
        model_info.download_url.clone()
    } else if model_info.download_url.contains("/tree/main") {
        model_info
            .download_url
            .replace("/tree/main/", "/resolve/main/")
            .replace("/tree/main", "/resolve/main")
    } else {
        format!(
            "{}/resolve/main",
            model_info.download_url.trim_end_matches('/')
        )
    };

    let other_files = [
        ("config.json", format!("{}/config.json", download_base)),
        (
            "tokenizer.json",
            format!("{}/tokenizer.json", download_base),
        ),
        (
            "vocabulary.txt",
            format!("{}/vocabulary.txt", download_base),
        ),
        (
            "preprocessor_config.json",
            format!("{}/preprocessor_config.json", download_base),
        ),
    ];

    for (filename, url) in &other_files {
        let file_path = model_dir.join(filename);
        if file_path.exists() {
            log_debug(format!("  ✓ {} already exists", filename));
            continue;
        }
        log_debug(format!("📥 Downloading {}", filename));
        let response = client.get(url).send().await?;
        let bytes = response.bytes().await?;
        tokio::fs::write(&file_path, bytes).await?;
        log_debug(format!("  ✓ {} downloaded", filename));
    }
    Ok(())
}

fn resolve_legacy_id(
    model_type: &Option<String>,
    model_info: &crate::models::WhisperModelInfo,
) -> String {
    if let Some(model_type_str) = model_type {
        model_type_str.clone()
    } else {
        match model_info.model_name.as_str() {
            name if name.contains("mukowaty/faster-whisper-int8") => "large",
            "Zoont/faster-whisper-large-v3-turbo-int8-ct2" => "large-v3",
            "guillaumekln/faster-whisper-large-v2" => "large-v2",
            _ => "large",
        }
        .to_string()
    }
}
