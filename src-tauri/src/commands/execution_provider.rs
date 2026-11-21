// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Tauri commands for execution provider management.
//!
//! With ort's `load-dynamic` feature, ort loads ORT at runtime and detects
//! available devices natively.
//!
//! Commands:
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
/// The frontend uses this to display the active device in Settings.
#[tauri::command]
pub async fn list_ort_devices() -> Result<Vec<DeviceInfo>, String> {
    // list_devices() touches ORT's global environment which may block while
    // a model preload is in progress. Run on the blocking pool.
    tokio::task::spawn_blocking(list_devices)
        .await
        .map_err(|e| format!("Task join error: {}", e))
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
        "coreml" | "core_ml" => ExecutionProviderType::CoreMl,
        "cpu" => ExecutionProviderType::Cpu,
        _ => return Err(format!("Unknown execution provider: {}", provider)),
    };

    AppSettings::update(|s| s.execution_provider = Some(provider.clone())).to_cmd()?;

    log::info!("Set preferred execution provider to: {}", provider);

    Ok(())
}
