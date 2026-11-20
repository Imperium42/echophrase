// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Stream + extract + verify pipeline for the CUDA ORT plugin.
//!
//! Follows the same pattern as `commands/transcription.rs::download_single_file`
//! (reqwest streaming, chunk loop, throttled progress emits, cancellation flag)
//! but tailored to a single-archive download that we extract and filter. Intentionally
//! kept separate rather than refactoring the model downloader: the model path has
//! multi-file + HF + resume concerns that don't apply here, and merging them would
//! either inflate this module or make the model path riskier to touch.
//!
//! Frontend event shape mirrors `"model-download-progress"` so `MiniDownloadProgress`
//! can be reused as-is with different event names.

use futures_util::StreamExt;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

const PROGRESS_EVENT: &str = "ort-plugin-progress";
const COMPLETE_EVENT: &str = "ort-plugin-complete";
const ERROR_EVENT: &str = "ort-plugin-error";

static CANCELLED: AtomicBool = AtomicBool::new(false);

pub fn cancel() {
    CANCELLED.store(true, Ordering::SeqCst);
}

pub async fn install(app: AppHandle) -> Result<(), String> {
    CANCELLED.store(false, Ordering::SeqCst);
    let result = install_inner(&app).await;
    if let Err(ref msg) = result {
        let _ = app.emit(ERROR_EVENT, msg.clone());
    }
    result
}

async fn install_inner(app: &AppHandle) -> Result<(), String> {
    let plugin_dir = super::plugin_dir(app)?;
    let tmp_dir = plugin_dir.with_extension("tmp");

    // Clean any leftover from a prior failed attempt so we start deterministic.
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir).map_err(|e| format!("remove stale tmp: {e}"))?;
    }
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("create tmp dir: {e}"))?;

    let zip_path = tmp_dir.join("plugin.zip");

    // Phase 1: download
    download_with_progress(app, super::PLUGIN_URL, &zip_path).await?;
    check_cancelled()?;

    // Phase 2: verify the download against the pinned SHA256 BEFORE anything
    // is extracted or test-loaded. A mismatch means a corrupted or tampered
    // asset - clean up and abort.
    let actual = hash_file(&zip_path)?;
    log::info!("[ort-plugin] downloaded zip SHA256: {actual}");
    if !actual.eq_ignore_ascii_case(super::PLUGIN_SHA256) {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        return Err(format!(
            "SHA256 mismatch: expected {} got {actual}. \
             The download may be corrupted or tampered with - aborting install.",
            super::PLUGIN_SHA256
        ));
    }

    // Phase 3: extract the 3 DLLs we keep, discard everything else.
    let _ = app.emit(
        PROGRESS_EVENT,
        json!({
            "progress": 100,
            "status": "Extracting",
            "stage": "extract",
        }),
    );
    extract_kept_files(&zip_path, &tmp_dir)?;
    check_cancelled()?;

    // Phase 3.5: verify the CUDA provider DLL can actually load on THIS system
    // before we tell the user to restart. Catches CUDA major-version mismatch
    // (eg user has CUDA 13, but ORT 1.24.2's provider DLL imports cudart64_12.dll
    // by name), missing dependencies, unsupported GPU arch, etc.
    let provider_dll = tmp_dir.join("onnxruntime_providers_cuda.dll");
    verify_loadable(&provider_dll).map_err(|e| {
        // Clean up tmp so the user isn't left with 600MB of unusable files.
        let _ = std::fs::remove_dir_all(&tmp_dir);
        format!(
            "Plugin downloaded, but the CUDA runtime can't load on this system: {e}\n\n\
             Most likely cause: CUDA version mismatch. ORT 1.24.2's CUDA plugin is built \
             against CUDA 12.x. If your CUDA_PATH points at CUDA 13.x, install CUDA 12.x \
             alongside it (both versions can coexist) and ensure CUDA_PATH or a \
             CUDA_PATH_V12_* variable points at the 12.x install."
        )
    })?;

    // Write VERSION marker for future upgrade detection.
    std::fs::write(tmp_dir.join("VERSION"), super::PLUGIN_VERSION)
        .map_err(|e| format!("write VERSION: {e}"))?;

    // Zip is no longer needed - remove before swap so it doesn't end up in the
    // final plugin dir (which would waste ~650MB).
    let _ = std::fs::remove_file(&zip_path);

    // Phase 4: atomic-ish swap. Remove old plugin dir (if any), rename tmp to final.
    if plugin_dir.exists() {
        std::fs::remove_dir_all(&plugin_dir).map_err(|e| format!("remove old plugin dir: {e}"))?;
    }
    if let Some(parent) = plugin_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create plugin parent: {e}"))?;
    }
    std::fs::rename(&tmp_dir, &plugin_dir).map_err(|e| format!("rename tmp -> plugin dir: {e}"))?;

    let _ = app.emit(COMPLETE_EVENT, json!({ "version": super::PLUGIN_VERSION }));
    Ok(())
}

async fn download_with_progress(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HTTP request: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} for {url}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("create zip file: {e}"))?;
    let mut stream = resp.bytes_stream();

    let start = tokio::time::Instant::now();
    let mut last_emit = start;
    let mut downloaded: u64 = 0;
    // 10-sample moving window for smoothed speed (matches transcription.rs pattern)
    let mut samples: std::collections::VecDeque<(tokio::time::Instant, u64)> = Default::default();
    samples.push_back((start, 0));

    while let Some(chunk) = stream.next().await {
        check_cancelled()?;
        let chunk = chunk.map_err(|e| format!("read chunk: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("write chunk: {e}"))?;
        downloaded += chunk.len() as u64;

        let now = tokio::time::Instant::now();
        if now.duration_since(last_emit) >= std::time::Duration::from_millis(150) {
            last_emit = now;
            samples.push_back((now, downloaded));
            while samples.len() > 10 {
                samples.pop_front();
            }
            let (then, then_bytes) = *samples.front().unwrap();
            let dt = now.duration_since(then).as_secs_f64().max(0.001);
            let bps = (downloaded.saturating_sub(then_bytes)) as f64 / dt;
            let progress = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0).min(99.0) as u32
            } else {
                0
            };
            let remaining = if bps > 1.0 && total > downloaded {
                format_secs(((total - downloaded) as f64 / bps) as u64)
            } else {
                String::new()
            };
            let elapsed = format_secs(now.duration_since(start).as_secs());
            let _ = app.emit(
                PROGRESS_EVENT,
                json!({
                    "progress": progress,
                    "bytesDownloaded": downloaded,
                    "totalBytes": total,
                    "speed": format_bps(bps),
                    "timeRemaining": remaining,
                    "timeElapsed": elapsed,
                    "status": "Downloading",
                    "stage": "download",
                }),
            );
        }
    }
    file.flush().await.map_err(|e| format!("flush zip: {e}"))?;
    Ok(())
}

fn extract_kept_files(zip_path: &Path, out_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("parse zip: {e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("zip entry {i}: {e}"))?;
        let name = entry.name().to_string();
        let Some(file_name) = Path::new(&name).file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !super::KEEP_FILES.contains(&file_name) {
            continue;
        }
        let dest = out_dir.join(file_name);
        let mut out =
            std::fs::File::create(&dest).map_err(|e| format!("create {file_name}: {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract {file_name}: {e}"))?;
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("open for hash: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read for hash: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn check_cancelled() -> Result<(), String> {
    if CANCELLED.load(Ordering::SeqCst) {
        Err("Cancelled".to_string())
    } else {
        Ok(())
    }
}

fn format_bps(bps: f64) -> String {
    if bps >= 1_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else if bps >= 1_000.0 {
        format!("{:.0} KB/s", bps / 1_000.0)
    } else {
        format!("{bps:.0} B/s")
    }
}

/// Test-load the CUDA provider DLL to confirm all its transitive deps resolve on
/// this machine. Preloads cuDNN + CUDA runtime DLLs first so Windows has handles
/// ready by bare name. Returns clean Ok when Library::new succeeds; detailed Err
/// with the GetLastError-derived reason otherwise.
fn verify_loadable(provider_dll: &Path) -> Result<(), String> {
    crate::preload_cuda_deps();
    let lib = unsafe { libloading::Library::new(provider_dll) }
        .map_err(|e| format!("LoadLibraryExW failed: {e}"))?;
    drop(lib);
    Ok(())
}

fn format_secs(s: u64) -> String {
    if s >= 60 {
        format!("{}m {}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}
