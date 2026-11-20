// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use anyhow::Result;

// ============================================================================
// Backend Type Detection
// ============================================================================

/// Supported transcription backend types
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackendType {
    /// Candle ML framework for Whisper models (safetensors format)
    Candle,
    /// ONNX Runtime for NVIDIA Parakeet and other ONNX models
    Onnx,
}

#[allow(dead_code)]
impl BackendType {
    /// Auto-detect backend type from model ID
    ///
    /// This matches the MODEL_REGISTRY in frontend's types/models.ts:
    /// - Candle backend: whisper-*, distil-* models (safetensors format)
    /// - ONNX backend: parakeet-*, *-onnx models, custom models
    pub fn from_model_id(model_id: &str) -> Self {
        // ONNX models (NVIDIA Parakeet, explicit ONNX, custom)
        if model_id.contains("parakeet")
            || model_id.contains("-onnx")
            || model_id.ends_with(".onnx")
            || model_id == "custom"
        {
            return BackendType::Onnx;
        }

        // Default to Candle for Whisper-family models
        BackendType::Candle
    }

    /// Check if model uses ONNX format
    pub fn is_onnx(&self) -> bool {
        matches!(self, BackendType::Onnx)
    }

    /// Check if model uses Candle (safetensors)
    pub fn is_candle(&self) -> bool {
        matches!(self, BackendType::Candle)
    }
}

// ============================================================================
// Transcription Backend Trait
// ============================================================================

/// Transcription backend interface for swappable implementations
///
/// This trait allows easy switching between different transcription engines
/// (Candle, ONNX Runtime, future backends) without changing application code.
///
/// Backends implement ONE required method (samples in, text out); callers use
/// the provided [`TranscriptionBackend::transcribe`] entry point, which
/// resolves any [`echo_audio::AudioSource`] (file on disk OR in-memory buffer)
/// to the canonical 16 kHz mono format first. Backends never do their own
/// audio I/O.
///
/// Implementations must be thread-safe (Send + Sync) for use with Tauri commands.
pub trait TranscriptionBackend: Send + Sync {
    /// Required: transcribe raw audio samples.
    ///
    /// # Arguments
    /// * `samples` - f32 samples, 16 kHz mono, normalized to [-1.0, 1.0]
    fn transcribe_samples(
        &self,
        samples: &[f32],
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    /// Backend name for logging and debugging
    /// (e.g., "Candle (Pure Rust)", "Parakeet TDT v3 (parakeet-rs)")
    fn backend_name(&self) -> &'static str;

    /// Provided high-level entry point: resolve `source` (file or memory) to
    /// 16 kHz mono samples via echo-audio, then transcribe. This is the only
    /// method application code should call.
    fn transcribe(
        &self,
        source: &echo_audio::AudioSource,
    ) -> impl std::future::Future<Output = Result<String>> + Send {
        async move {
            let samples = source.samples_mono_16k()?;
            self.transcribe_samples(&samples).await
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_type_detection_candle() {
        // Whisper models should use Candle
        assert_eq!(
            BackendType::from_model_id("distil-large-v3"),
            BackendType::Candle
        );
        assert_eq!(
            BackendType::from_model_id("whisper-large-v3-turbo"),
            BackendType::Candle
        );
        assert_eq!(
            BackendType::from_model_id("whisper-medium"),
            BackendType::Candle
        );
        assert_eq!(
            BackendType::from_model_id("whisper-small"),
            BackendType::Candle
        );
    }

    #[test]
    fn test_backend_type_detection_onnx() {
        // NVIDIA Parakeet and explicit ONNX models
        assert_eq!(
            BackendType::from_model_id("parakeet-tdt-0.6b-v2"),
            BackendType::Onnx
        );
        assert_eq!(
            BackendType::from_model_id("whisper-large-v3-onnx"),
            BackendType::Onnx
        );
        assert_eq!(BackendType::from_model_id("model.onnx"), BackendType::Onnx);
        assert_eq!(BackendType::from_model_id("custom"), BackendType::Onnx);
    }

    #[test]
    fn test_backend_type_helpers() {
        let candle = BackendType::Candle;
        let onnx = BackendType::Onnx;

        assert!(candle.is_candle());
        assert!(!candle.is_onnx());
        assert!(onnx.is_onnx());
        assert!(!onnx.is_candle());
    }
}
