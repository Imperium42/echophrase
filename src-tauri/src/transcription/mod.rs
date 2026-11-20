// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

pub mod backend;
// pub mod ct2_whisper;  // Disabled - migrating to Candle backend
pub mod candle_whisper;
pub mod chunking;
pub mod execution_provider;
pub mod idle;
pub mod onnx_backend;
pub mod qwen3_asr;
pub mod silence;

// Re-exports for external use (may not be used internally)
#[allow(unused_imports)]
pub use backend::{BackendType, TranscriptionBackend};
#[allow(unused_imports)]
pub use execution_provider::{
    ExecutionProviderSelection, ExecutionProviderStatus, ExecutionProviderType,
    build_execution_providers, get_execution_provider_status, get_platform_default,
};
#[allow(unused_imports)]
pub use onnx_backend::{OnnxModelType, OnnxRuntimeBackend};

use echo_audio::AudioSource;

/// High-level transcription router: resolve `source` (file OR in-memory
/// buffer) and dispatch to the backend selected in settings.
///
/// This is the single entry point commands use - the Qwen3/ONNX/Candle
/// three-way dispatch lives here and nowhere else. Pass `app` to enable
/// Candle's realtime preview events; `None` runs preview-less.
pub async fn transcribe_source(
    source: &AudioSource,
    app: Option<&tauri::AppHandle>,
) -> anyhow::Result<String> {
    use crate::models::is_onnx_model;
    use silence::{SILENCE_PEAK_THRESHOLD, SILENCE_RESULT, is_known_silence_hallucination};

    // Defer background telemetry flushes: no network request should be
    // observable near a transcription (see telemetry::client IDLE_FLUSH_THRESHOLD).
    crate::telemetry::mark_activity();

    // Keeps the model from being idle-unloaded mid-transcription, and restarts
    // the idle clock when this returns (on every path).
    let _activity = idle::begin();

    let settings = crate::settings::AppSettings::load()
        .map_err(|e| anyhow::anyhow!("Failed to load settings: {e}"))?;
    let model_id = &settings.model;

    // Level-based silence guard (primary): skip the model call entirely when
    // the captured audio is (near-)silent. This is the actual root cause
    // behind fixed-string hallucinations ("Current.", "None", ...) - Qwen3-ASR
    // and friends have to output *something* for near-zero input. Cheap to
    // check up front and avoids a wasted inference on top of a bad result.
    if let Ok(samples) = source.samples_mono_16k() {
        let level = echo_audio::sample::peak(&samples);
        if level < SILENCE_PEAK_THRESHOLD {
            log::info!(
                "transcribe_source: near-silent input (peak={level:.6} < {SILENCE_PEAK_THRESHOLD}), skipping ASR"
            );
            return Ok(SILENCE_RESULT.to_string());
        }
    }

    let text = if model_id.starts_with("qwen3-asr") {
        transcribe_via_qwen3(source, model_id).await
    } else if model_id == "custom" || is_onnx_model(model_id) {
        // BYO custom models are ONNX-only (import only accepts .onnx files),
        // but "custom" is a sentinel, not a MODEL_REGISTRY entry - is_onnx_model's
        // registry lookup can never match it, so it needs its own branch here.
        // Falling through to Candle otherwise silently misroutes every custom
        // model to the Whisper backend.
        transcribe_via_onnx(source, model_id).await
    } else {
        transcribe_via_candle(source, app).await
    }?;

    // Known-hallucination backstop: the level check didn't trip (e.g. a brief
    // click or low room tone) but the model returned only known silence
    // tokens. Treat it the same as silence rather than pasting garbage.
    let text = if is_known_silence_hallucination(&text) {
        log::info!(
            "transcribe_source: ASR returned known silence hallucination ({text:?}), treating as silence"
        );
        SILENCE_RESULT.to_string()
    } else {
        text
    };

    // Optional Simplified <-> Traditional Chinese conversion, applied right
    // after ASR and before any grammar transform (which is English-only).
    Ok(crate::transform::convert_chinese_script(
        text,
        &settings.chinese_script,
    ))
}

/// Qwen3-ASR backend (multilingual, hand-rolled ORT sessions with KV-cache
/// decode). Sibling of the parakeet ONNX path, guarded by ORT availability.
async fn transcribe_via_qwen3(source: &AudioSource, model_id: &str) -> anyhow::Result<String> {
    if !crate::is_onnx_runtime_available() {
        anyhow::bail!(
            "ONNX Runtime is not available on this system. \
             ONNX models require macOS 13.3+ or a compatible Linux/Windows system."
        );
    }
    let backend = qwen3_asr::Qwen3AsrBackend::new(model_id);
    log::info!(
        "Using backend: {} (model: {})",
        backend.backend_name(),
        model_id
    );
    backend.transcribe(source).await
}

/// ONNX Runtime backend (Parakeet etc.), guarded by ORT availability.
async fn transcribe_via_onnx(source: &AudioSource, model_id: &str) -> anyhow::Result<String> {
    if !crate::is_onnx_runtime_available() {
        anyhow::bail!(
            "ONNX Runtime is not available on this system. \
             ONNX models require macOS 13.3+ or a compatible Linux/Windows system."
        );
    }
    let backend = OnnxRuntimeBackend::new(model_id);
    log::info!(
        "Using backend: {} (model: {})",
        backend.backend_name(),
        model_id
    );
    backend.transcribe(source).await
}

/// Candle (Whisper) backend; with an AppHandle it emits realtime preview events.
async fn transcribe_via_candle(
    source: &AudioSource,
    app: Option<&tauri::AppHandle>,
) -> anyhow::Result<String> {
    match app {
        Some(app) => {
            log::info!("Using backend: Candle (Pure Rust) with realtime preview");
            let samples = source.samples_mono_16k()?;
            candle_whisper::transcribe_audio_buffer_with_preview(&samples, Some(app.clone())).await
        }
        None => {
            let backend = candle_whisper::CandleWhisperBackend;
            log::info!("Using backend: {}", backend.backend_name());
            backend.transcribe(source).await
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod real_test;

#[cfg(test)]
mod minimal_test;
