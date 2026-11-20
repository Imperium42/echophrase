// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! VAD (Voice Activity Detection) Tauri commands
//!
//! Provides commands for testing audio input with live VAD feedback.
//! Used in the Audio Input settings section.

use crate::error::ToCmdErr;
use crate::vad::{self, VadTestState};
use std::sync::Arc;
use tauri::State;

/// Managed state for VAD test
pub struct VadState(pub Arc<VadTestState>);

/// Start VAD test mode - streams audio levels back to frontend
///
/// Emits "vad-level" events with:
/// - probability: VAD speech probability (0.0-1.0)
/// - is_speech: Whether above threshold
/// - peak_amplitude: For visual meter
/// - rms_level: Smoothed level
#[tauri::command]
pub async fn start_vad_test(
    app: tauri::AppHandle,
    state: State<'_, VadState>,
    device_id: String,
) -> Result<(), String> {
    vad::start_vad_test(app, state.0.clone(), device_id)
        .await
        .to_cmd()
}

/// Stop VAD test mode
#[tauri::command]
pub async fn stop_vad_test(state: State<'_, VadState>) -> Result<(), String> {
    vad::stop_vad_test(state.0.clone()).await.to_cmd()
}

/// Set VAD threshold (0-100 maps to 0.0-1.0)
#[tauri::command]
pub fn set_vad_threshold(state: State<'_, VadState>, threshold: u8) -> Result<(), String> {
    state.0.set_threshold(threshold);
    Ok(())
}

/// Check if VAD test is currently running
#[tauri::command]
pub fn is_vad_test_running(state: State<'_, VadState>) -> bool {
    state.0.is_running()
}
