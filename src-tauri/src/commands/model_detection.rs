// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Tauri commands for model auto-detection.
//!
//! These commands handle first-launch model selection based on GPU VRAM.

use crate::commands::recording::get_system_info;
use crate::error::ToCmdErr;
use crate::model_detection::is_non_english_system_locale;
use crate::model_detection::{ModelSelection, VramTier, select_optimal_model};
use crate::models::is_onnx_model;
use crate::settings::{AppSettings, DEFAULT_MODEL_ID};
use serde::Serialize;

/// Result returned to frontend after auto-detection.
#[derive(Debug, Clone, Serialize)]
pub struct AutoDetectionResult {
    /// Selected model ID
    pub model_id: String,
    /// Detected VRAM in MB (if available)
    pub vram_mb: Option<u32>,
    /// VRAM tier classification
    pub vram_tier: VramTier,
    /// Human-readable reason for selection
    pub reason: String,
    /// Whether auto-detection was performed
    pub was_auto_detected: bool,
}

impl From<ModelSelection> for AutoDetectionResult {
    fn from(selection: ModelSelection) -> Self {
        Self {
            model_id: selection.model_id,
            vram_mb: selection.vram_mb,
            vram_tier: selection.vram_tier,
            reason: selection.reason,
            was_auto_detected: true,
        }
    }
}

/// Check if this is first launch and auto-detect model if needed.
///
/// Returns `None` if settings already exist with a manually-selected model.
/// Returns `Some(result)` if auto-detection was performed.
///
/// Detection is skipped when:
/// - User has manually changed the model from default
/// - Auto-detection has already run (auto_detected_model = true)
#[tauri::command]
pub async fn maybe_auto_detect_model() -> Result<Option<AutoDetectionResult>, String> {
    // Load or create settings
    let settings = AppSettings::load().to_cmd()?;
    let ort_available = crate::is_onnx_runtime_available();

    // Force re-detection if current model requires ORT but ORT is unavailable.
    // This handles macOS < 13.3 where ORT can't load (missing C++20 symbols) -
    // the user's selected ONNX model (e.g. parakeet-eou-120m) would always fail,
    // so we auto-switch to a compatible Candle/safetensors model instead.
    let force_ort_redetect = is_onnx_model(&settings.model) && !ort_available;
    if force_ort_redetect {
        log::warn!(
            "Model auto-detection: current model '{}' requires ONNX Runtime which is unavailable - forcing re-detection",
            settings.model
        );
    }

    // Skip if auto-detection was already done (unless ORT forces re-detection)
    if settings.auto_detected_model && !force_ort_redetect {
        log::info!("Model auto-detection: skipping (already auto-detected)");
        return Ok(None);
    }

    // Skip if user has manually selected a non-default model (unless ORT forces re-detection)
    if settings.model != DEFAULT_MODEL_ID && !force_ort_redetect {
        log::info!(
            "Model auto-detection: skipping (user selected {} manually)",
            settings.model
        );
        return Ok(None);
    }

    // Run detection
    log::info!(
        "Model auto-detection: running detection (force_ort_redetect={})",
        force_ort_redetect
    );

    // Get GPU info
    let system_info = get_system_info().await.to_cmd()?;
    let vram_mb = system_info.gpu_vram_mb;

    log::info!(
        "Model auto-detection: GPU={:?}, VRAM={:?}MB, ORT={}",
        system_info.gpu_name,
        vram_mb,
        ort_available
    );

    // Run selection algorithm
    let selection = select_optimal_model(vram_mb, ort_available, is_non_english_system_locale());

    log::info!(
        "Model auto-detection: selected {} (tier: {:?}, reason: {})",
        selection.model_id,
        selection.vram_tier,
        selection.reason
    );

    // Simplified-Chinese systems write Simplified, so the global Traditional
    // default would convert their own script away from them. Applied here (and
    // NOT in force_auto_detect_model) because this command runs once, gated on
    // auto_detected_model - re-applying it on every launch would silently
    // revert a user who deliberately picked Traditional.
    let prefers_simplified = crate::model_detection::system_prefers_simplified_chinese();
    if prefers_simplified {
        log::info!(
            "Model auto-detection: Simplified-Chinese locale - defaulting script to Simplified"
        );
    }

    // Update settings
    let detected_model = selection.model_id.clone();
    AppSettings::update(|s| {
        s.model = detected_model;
        s.auto_detected_model = true;
        if prefers_simplified {
            s.chinese_script = "simplified".to_string();
        }
    })
    .to_cmd()?;

    Ok(Some(selection.into()))
}

/// Force re-detection of optimal model.
///
/// Useful for settings reset, hardware changes, or debugging.
/// Always runs detection regardless of previous state.
#[tauri::command]
pub async fn force_auto_detect_model() -> Result<AutoDetectionResult, String> {
    log::info!("Model auto-detection: forced re-detection");

    // Get GPU info
    let system_info = get_system_info().await.to_cmd()?;
    let vram_mb = system_info.gpu_vram_mb;
    let ort_available = crate::is_onnx_runtime_available();

    log::info!(
        "Model auto-detection: GPU={:?}, VRAM={:?}MB, ORT={}",
        system_info.gpu_name,
        vram_mb,
        ort_available
    );

    // Run selection algorithm
    let selection = select_optimal_model(vram_mb, ort_available, is_non_english_system_locale());

    log::info!(
        "Model auto-detection: selected {} (tier: {:?}, reason: {})",
        selection.model_id,
        selection.vram_tier,
        selection.reason
    );

    // Update settings
    let detected_model = selection.model_id.clone();
    AppSettings::update(|s| {
        s.model = detected_model;
        s.auto_detected_model = true;
    })
    .to_cmd()?;

    Ok(selection.into())
}

/// Get current GPU info and recommended model without changing settings.
///
/// Useful for displaying recommendations in the UI.
#[tauri::command]
pub async fn get_model_recommendation() -> Result<AutoDetectionResult, String> {
    let system_info = get_system_info().await.to_cmd()?;
    let selection = select_optimal_model(
        system_info.gpu_vram_mb,
        crate::is_onnx_runtime_available(),
        is_non_english_system_locale(),
    );

    Ok(AutoDetectionResult {
        model_id: selection.model_id,
        vram_mb: selection.vram_mb,
        vram_tier: selection.vram_tier,
        reason: selection.reason,
        was_auto_detected: false, // Just a recommendation, not applied
    })
}

/// Reset model detection state to trigger re-detection on next launch.
///
/// This is useful for the debug reset button.
#[tauri::command]
pub async fn reset_model_detection() -> Result<(), String> {
    AppSettings::update(|s| {
        s.auto_detected_model = false;
        s.model = DEFAULT_MODEL_ID.to_string();
    })
    .to_cmd()?;

    log::info!("Model auto-detection: reset to defaults");
    Ok(())
}
