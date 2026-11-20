// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Transform Commands
//!
//! Tauri commands for post-processing transcription with LLM.

use crate::transform::T5GrammarCorrector;

/// Apply grammar correction to text
#[tauri::command]
pub async fn transform_text(text: String, rules: Option<String>) -> Result<String, String> {
    // TODO: Use rules parameter for custom prompts (requires prompt-based model)
    // For now, T5 grammar model is fine-tuned specifically for grammar correction
    let _ = rules;

    tokio::task::spawn_blocking(move || T5GrammarCorrector::correct(&text))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| format!("Transform error: {}", e))
}

/// Preload the transform model
#[tauri::command]
pub async fn preload_transform_model() -> Result<(), String> {
    tokio::task::spawn_blocking(T5GrammarCorrector::preload)
        .await
        .map_err(|e| format!("Task join error: {}", e))?
        .map_err(|e| format!("Preload error: {}", e))
}

/// Check if transform model is loaded
#[tauri::command]
pub fn is_transform_model_loaded() -> bool {
    T5GrammarCorrector::is_loaded()
}

/// Unload transform model to free memory
#[tauri::command]
pub fn unload_transform_model() {
    T5GrammarCorrector::unload();
}
