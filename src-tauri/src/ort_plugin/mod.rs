// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! CUDA ORT runtime plugin management (Windows only).
//!
//! The installer bundles DirectML ORT (~15MB) as the default Windows runtime.
//! Users who want CUDA acceleration opt into downloading the CUDA ORT build as
//! a plugin (~650MB, NVIDIA only). Plugin DLLs live in
//! `%LOCALAPPDATA%/com.imperium42.echophrase/ort/cuda/`, outside the installed
//! app dir so they survive app updates.
//!
//! Activation happens at startup in `select_ort_runtime()` (lib.rs), which sets
//! `ORT_DYLIB_PATH` to the plugin DLL before any ORT API is touched.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
pub mod downloader;

/// ORT version the plugin expects. When Microsoft ships a new ORT release we
/// bump this, update `PLUGIN_URL`, and refill `PLUGIN_SHA256`.
pub const PLUGIN_VERSION: &str = "1.24.2";

/// Canonical source: Microsoft's GitHub release. Keeping this as the direct
/// asset URL avoids any proxy/caching layer (see plan: user picked GitHub
/// official + strip locally over hosting our own mirror).
pub const PLUGIN_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-gpu-1.24.2.zip";

/// SHA256 of the asset. Verified against both the GitHub release API digest
/// and an independent local download (2026-07-03). Verification is mandatory -
/// a mismatch aborts the install before anything is extracted or dlopen'd.
pub const PLUGIN_SHA256: &str = "4152b67ea67405cf642fdc484a4e24fb1d9bfda7a78dfd5f756c73812a40bdcd";

/// Files kept after extracting the zip. Everything else (TensorRT, PDBs,
/// headers, license text) is discarded to save ~50-100MB of disk.
pub const KEEP_FILES: &[&str] = &[
    "onnxruntime.dll",
    "onnxruntime_providers_shared.dll",
    "onnxruntime_providers_cuda.dll",
];

#[derive(Debug, Serialize, Clone)]
pub struct PluginStatus {
    pub installed: bool,
    pub version: Option<String>,
    /// True when the currently-loaded ORT runtime IS the plugin (ie CUDA is
    /// live in this process). After a fresh install the user must restart the
    /// app to flip this from false to true.
    pub active_in_current_session: bool,
    /// True when the plugin's CUDA provider DLL can actually load on this
    /// machine (deps resolve, CUDA major version matches). When false, the
    /// plugin is installed but unusable - UI should surface an error instead
    /// of prompting restart.
    pub compatible: bool,
    /// Absolute path of the plugin directory (displayed in the UI for support/debug).
    pub plugin_dir: String,
}

/// Test-load the CUDA provider DLL to determine whether the plugin can run on
/// this machine. Returns true on success; logs and returns false on failure.
///
/// Callers must have already preloaded the CUDA transitive deps (cuDNN, cudart,
/// cublas, ...) before invoking this - otherwise `LoadLibraryExW` can't resolve
/// the provider's imports and we'd report a false negative.
#[cfg(target_os = "windows")]
pub fn can_plugin_load(provider_dll: &std::path::Path) -> bool {
    if !provider_dll.is_file() {
        return false;
    }
    match unsafe { libloading::Library::new(provider_dll) } {
        Ok(lib) => {
            drop(lib);
            true
        }
        Err(e) => {
            eprintln!(
                "[ort_plugin] CUDA provider DLL won't load ({}): {e}",
                provider_dll.display()
            );
            false
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn can_plugin_load(_provider_dll: &std::path::Path) -> bool {
    false
}

/// Resolve `%LOCALAPPDATA%/com.imperium42.echophrase/ort/cuda/`.
pub fn plugin_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|e| format!("resolve app_local_data_dir: {e}"))
        .map(|d| d.join("ort").join("cuda"))
}

/// True when the given path is underneath the plugin dir. Used at startup by
/// `select_ort_runtime()` to detect whether the chosen runtime is the plugin.
pub fn is_plugin_path(path: &std::path::Path, plugin_dir: &std::path::Path) -> bool {
    path.starts_with(plugin_dir)
}

#[tauri::command]
pub async fn get_cuda_plugin_status(app: AppHandle) -> Result<PluginStatus, String> {
    let dir = plugin_dir(&app)?;
    let main_dll = dir.join("onnxruntime.dll");
    let provider_dll = dir.join("onnxruntime_providers_cuda.dll");
    let version_file = dir.join("VERSION");
    let installed = main_dll.is_file();
    let version = std::fs::read_to_string(&version_file)
        .ok()
        .map(|s| s.trim().to_string());

    let active_in_current_session = std::env::var("ORT_DYLIB_PATH")
        .ok()
        .map(PathBuf::from)
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .map(|p| p == dir)
        .unwrap_or(false);

    // Compatible = provider DLL test-loads. If the app is already using the
    // plugin (active=true), by definition it loaded at startup -> compatible.
    // Otherwise we do a fresh test-load so the UI can differentiate "installed
    // and ready, just needs restart" from "installed but broken on this machine".
    let compatible = if installed {
        if active_in_current_session {
            true
        } else {
            can_plugin_load(&provider_dll)
        }
    } else {
        false
    };

    Ok(PluginStatus {
        installed,
        version,
        active_in_current_session,
        compatible,
        plugin_dir: dir.display().to_string(),
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn install_cuda_plugin(app: AppHandle) -> Result<(), String> {
    downloader::install(app).await
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn install_cuda_plugin(_app: AppHandle) -> Result<(), String> {
    Err("CUDA plugin is Windows-only".to_string())
}

#[tauri::command]
pub async fn cancel_cuda_plugin_install() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    downloader::cancel();
    Ok(())
}

#[tauri::command]
pub async fn uninstall_cuda_plugin(app: AppHandle) -> Result<(), String> {
    let dir = plugin_dir(&app)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("remove {}: {e}", dir.display()))?;
    }
    Ok(())
}
