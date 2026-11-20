// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Domain-specific error types for Echophrase
//!
//! These errors serialize to structured JSON when crossing the Tauri command boundary,
//! allowing the frontend to parse error types and show actionable recovery messages.
//!
//! Use `thiserror` for domain errors that need to be inspected/handled programmatically.
//! Use `anyhow` for generic errors that just need to be logged/displayed.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

/**
 * Main error type for transcription operations
 *
 * The `tag = "type", content = "details"` serde attribute produces JSON like:
 * {
 *   "type": "GpuUnavailable",
 *   "details": {
 *     "reason": "CoreML not supported on this macOS version",
 *     "provider": "CoreML",
 *     "fallback_available": true
 *   }
 * }
 */
#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum TranscriptionError {
    #[error("GPU unavailable: {reason}")]
    GpuUnavailable {
        reason: String,
        provider: String, // "CoreML", "Metal", "CUDA", "DirectML"
        fallback_available: bool,
    },

    #[error("ONNX Runtime library not found: {path}")]
    OrtLibraryNotFound {
        path: String,
        architecture: String, // "x86_64", "arm64", "x64"
        download_url: String,
    },

    #[error("ONNX Runtime version mismatch")]
    OrtVersionMismatch {
        found_version: Option<String>,
        required_version: String, // "1.23.2+"
        path: String,
    },

    #[error("Model not loaded: {model_id}")]
    ModelNotLoaded {
        model_id: String,
        required_vram_mb: Option<u32>,
        error_details: Option<String>,
    },

    #[error("Model download failed: {model_id}")]
    ModelDownloadFailed {
        model_id: String,
        reason: String,
        retry_available: bool,
    },

    #[error("Audio processing failed: {reason}")]
    AudioProcessing {
        reason: String,
        file_path: Option<String>,
    },

    #[error("Insufficient VRAM: {required_mb}MB required, {available_mb}MB available")]
    InsufficientVram {
        required_mb: u32,
        available_mb: u32,
        suggested_model: Option<String>,
    },

    #[error("Inference failed: {reason}")]
    InferenceFailed {
        reason: String,
        model_id: String,
        recoverable: bool,
    },

    #[error("Configuration error: {message}")]
    Configuration {
        message: String,
        key: Option<String>,
    },

    #[error("Feature not available: {feature}")]
    FeatureUnavailable {
        feature: String,
        reason: String,
        platform: String,
    },

    // Bridge from anyhow for generic errors
    #[error("{0}")]
    Other(String),
}

/**
 * Error type for audio recording operations
 */
#[derive(Debug, Error, Serialize, Deserialize)]
#[serde(tag = "type", content = "details")]
pub enum RecordingError {
    #[error("Microphone not found")]
    MicrophoneNotFound { available_devices: Vec<String> },

    #[error("Recording already in progress")]
    AlreadyRecording,

    #[error("No active recording")]
    NotRecording,

    #[error("Device access denied")]
    PermissionDenied { platform: String, help_url: String },

    #[error("Audio device error: {message}")]
    DeviceError {
        message: String,
        device_name: Option<String>,
    },

    #[error("{0}")]
    Other(String),
}

// Implement From<anyhow::Error> for convenient error conversion
impl From<anyhow::Error> for TranscriptionError {
    fn from(err: anyhow::Error) -> Self {
        TranscriptionError::Other(err.to_string())
    }
}

impl From<anyhow::Error> for RecordingError {
    fn from(err: anyhow::Error) -> Self {
        RecordingError::Other(err.to_string())
    }
}

// Implement From<String> for backwards compatibility with Result<T, String>
impl From<String> for TranscriptionError {
    fn from(s: String) -> Self {
        TranscriptionError::Other(s)
    }
}

impl From<String> for RecordingError {
    fn from(s: String) -> Self {
        RecordingError::Other(s)
    }
}

/// Shorthand return type for Tauri commands that serialize errors as plain strings.
///
/// Use `CmdResult<T>` in `#[tauri::command]` functions instead of `Result<T, String>`
/// to keep signatures concise. The Tauri IPC boundary requires `E: Serialize`, and
/// `String` satisfies that.
pub type CmdResult<T> = Result<T, String>;

/// Extension trait that converts any `Result<T, E: Display>` into `CmdResult<T>`.
///
/// # Example
/// ```ignore
/// use crate::error::ToCmdErr;
///
/// fn load() -> CmdResult<u32> {
///     some_fallible_op().to_cmd()
/// }
/// ```
pub trait ToCmdErr<T> {
    /// Map the error variant to `String` via `Display`.
    fn to_cmd(self) -> CmdResult<T>;
}

impl<T, E: std::fmt::Display> ToCmdErr<T> for Result<T, E> {
    fn to_cmd(self) -> CmdResult<T> {
        self.map_err(|e| e.to_string())
    }
}

/**
 * Helper to convert PathBuf to String for serialization
 */
#[allow(dead_code)]
pub fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}
