// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Tauri commands for execution provider management.
//!
//! Since the ort 2.0.0-rc.12 upgrade with `load-dynamic`,
//! ort loads ORT at runtime and detects available devices natively. The
//! old commands for hand-rolled CUDA path probing (`save_cuda_paths`,
//! `get_cuda_addon_status`, `download_cuda_addon`, `check_ort_cuda_provider`,
//! `maybe_auto_enable_cuda`) have been deleted - they're no longer needed.
//!
//! Surviving commands:
//! - `get_execution_provider`        - report current EP status to the UI
//! - `set_preferred_execution_provider` - write the manual radio-button override
//! - `list_ort_devices`              - enumerate devices via ort native APIs

use crate::error::ToCmdErr;
use crate::settings::AppSettings;
use crate::transcription::execution_provider::{
    DeviceInfo, ExecutionProviderSelection, ExecutionProviderType, get_execution_provider_status,
    list_devices,
};

/// Get current execution provider status.
///
/// Returns information about which EP is active and what's available, based
/// on ort's native device enumeration.
#[tauri::command]
pub async fn get_execution_provider() -> Result<ExecutionProviderSelection, String> {
    let settings = AppSettings::load().to_cmd()?;
    // No ORT calls here -- status is built from settings + compiled-in knowledge.
    Ok(get_execution_provider_status(
        settings.execution_provider.as_deref(),
    ))
}

/// Enumerate every device ort can see on this system.
///
/// Replaces the old `find_cuda_installation` / `find_cudnn_in_cuda_dir`
/// hand-rolled scanning helpers. The frontend uses this to display
/// "Active: CUDA (NVIDIA GeForce RTX 3090)" or similar in Settings.
#[tauri::command]
pub async fn list_ort_devices() -> Result<Vec<DeviceInfo>, String> {
    // list_devices() touches ORT's global environment which may block while
    // a model preload is in progress. Run on the blocking pool.
    tokio::task::spawn_blocking(list_devices)
        .await
        .map_err(|e| format!("Task join error: {}", e))
}

/// Result of testing a single CUDA-related env var: the actual value (or empty
/// string if unset) + a boolean indicating whether it points somewhere sensible
/// (contains the expected subpath).
#[derive(Debug, serde::Serialize)]
pub struct CudaEnvVarTest {
    pub name: String,
    /// Current actual value of the env var; empty string if unset.
    pub actual: String,
    /// True when the actual value looks right (non-empty AND contains the
    /// expected install subpath like `NVIDIA GPU Computing Toolkit\CUDA\` or
    /// `NVIDIA\CUDNN\`). False means the UI should render this red.
    pub ok: bool,
}

/// Read CUDA_PATH and CUDNN_PATH env vars and verify each actually points at a
/// functional NVIDIA install by looking for a known marker file on disk.
///
/// Why a filesystem check instead of a substring match: the display hint uses
/// `%ProgramFiles%` as an *example*, but the user's actual env var could be a
/// totally different installer location (custom Program Files, D: drive, a
/// portable install, etc). What matters is whether the path resolves to a real
/// CUDA/cuDNN install, which we prove by checking for a canonical file:
///
/// - `CUDA_PATH`: `bin/nvcc.exe` is always present in every CUDA toolkit install
///   and its name doesn't change with CUDA major versions.
/// - `CUDNN_PATH`: recursive glob `**/cudnn64_*.dll`. cuDNN 9.x nests the
///   runtime DLL under `bin/<cuda_ver>/x64/` (eg `bin/13.2/x64/cudnn64_9.dll`),
///   while older layouts put it directly in `bin/`. A recursive glob handles
///   both without baking in version numbers.
///
/// `Path::join` + `Path::exists` accept both `/` and `\` separators on Windows
/// and do proper normalization, so we don't need manual string massaging.
///
/// Called from the CUDA Setup modal's "Test Paths" button.
#[tauri::command]
pub async fn test_cuda_env_paths() -> Result<Vec<CudaEnvVarTest>, String> {
    Ok(vec![
        test_path_with_marker_file("CUDA_PATH", "bin/nvcc.exe"),
        test_cudnn_path(),
    ])
}

/// Check that the env var points at a directory containing the given relative
/// PE binary file (and that the file really is a Windows PE binary, not a
/// zero-byte placeholder or a text file with the right name).
fn test_path_with_marker_file(name: &str, marker: &str) -> CudaEnvVarTest {
    let actual = std::env::var(name).unwrap_or_default();
    let ok = !actual.is_empty() && is_valid_pe_binary(&std::path::Path::new(&actual).join(marker));
    CudaEnvVarTest {
        name: name.to_string(),
        actual,
        ok,
    }
}

/// cuDNN check: recursively resolve any real `cudnn64_*.dll` under `<CUDNN_PATH>`.
///
/// Separate from the generic marker-file helper because cuDNN's DLL name is
/// version-suffixed (so we can't hard-code a filename) AND the install layout
/// nests it per-CUDA-version (`bin/13.2/x64/cudnn64_9.dll` on 9.x). We let the
/// `glob` crate resolve `<CUDNN_PATH>/**/cudnn64_*.dll` instead of hand-rolling
/// directory walks.
fn test_cudnn_path() -> CudaEnvVarTest {
    let actual = std::env::var("CUDNN_PATH").unwrap_or_default();
    let ok = !actual.is_empty() && find_cudnn_runtime_dll(&actual).is_some();
    CudaEnvVarTest {
        name: "CUDNN_PATH".to_string(),
        actual,
        ok,
    }
}

fn find_cudnn_runtime_dll(root: &str) -> Option<std::path::PathBuf> {
    let pattern = format!("{}/**/cudnn64_*.dll", root.trim_end_matches(['/', '\\']));
    glob::glob(&pattern)
        .ok()?
        .filter_map(|entry| entry.ok())
        .find(|path| is_valid_pe_binary(path))
}

/// Verify a file is actually a Windows PE binary (EXE or DLL), not a random
/// file with the right name. Two-step check:
///
/// 1. `metadata()` succeeds and the file is a regular file with a non-trivial
///    size (>= 1KB -- any real CUDA/cuDNN lib is at least hundreds of KB).
/// 2. First two bytes are `0x4D 0x5A` ("MZ"), the DOS stub signature every
///    PE/EXE/DLL on Windows starts with.
///
/// This catches the common failure modes: path points to a directory, an
/// empty placeholder, a leftover `.txt`, or a truncated/corrupted file.
fn is_valid_pe_binary(path: &std::path::Path) -> bool {
    use std::io::Read;

    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() < 1024 {
        return false;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 2];
    if file.read_exact(&mut magic).is_err() {
        return false;
    }
    magic == *b"MZ"
}

/// Set preferred execution provider (writes the manual radio-button override).
///
/// Pass `provider = "auto"` (or any unrecognized value) to clear the override
/// and let ort's `with_auto_device(MaxPerformance)` pick the best EP at session
/// creation time.
#[tauri::command]
pub async fn set_preferred_execution_provider(provider: String) -> Result<(), String> {
    if provider.eq_ignore_ascii_case("auto") || provider.is_empty() {
        AppSettings::update(|s| s.execution_provider = None).to_cmd()?;
        log::info!("Cleared execution provider override (auto-detect)");
        return Ok(());
    }

    // Validate against the known set
    let _ep_type = match provider.as_str() {
        "directml" | "direct_ml" => ExecutionProviderType::DirectMl,
        "cuda" => ExecutionProviderType::Cuda,
        "coreml" | "core_ml" => ExecutionProviderType::CoreMl,
        "cpu" => ExecutionProviderType::Cpu,
        _ => return Err(format!("Unknown execution provider: {}", provider)),
    };

    AppSettings::update(|s| s.execution_provider = Some(provider.clone())).to_cmd()?;

    log::info!("Set preferred execution provider to: {}", provider);

    Ok(())
}
