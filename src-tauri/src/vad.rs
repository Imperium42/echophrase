// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Voice Activity Detection module
//!
//! Provides real-time speech detection with configurable threshold.
//! Uses a trait-based interface for swappable VAD implementations.
//!
//! Used for:
//! - Live audio preview in settings
//! - Auto-stop recording on silence (future)

use anyhow::Result;
use ndarray::{Array1, Array2, Array3};
use ort::{session::Session, value::Tensor};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use tokio::sync::Mutex;

// ============================================================================
// VAD Backend Trait (Interface for swappable implementations)
// ============================================================================

/// Voice Activity Detection backend interface
///
/// Implementations can use different models or approaches:
/// - Silero VAD (ONNX)
/// - WebRTC VAD
/// - Custom neural networks
/// - Energy-based detection
pub trait VadBackend: Send {
    /// Predict speech probability for a chunk of audio
    ///
    /// # Arguments
    /// * `samples` - Audio samples (f32, 16kHz mono, 512 samples = 32ms)
    ///
    /// # Returns
    /// Speech probability (0.0 = silence, 1.0 = speech)
    #[allow(dead_code)]
    fn predict(&mut self, samples: &[f32]) -> f32;

    /// Reset internal state (call between recordings)
    #[allow(dead_code)]
    fn reset(&mut self);

    /// Backend name for logging
    #[allow(dead_code)]
    fn name(&self) -> &'static str;
}

// ============================================================================
// Silero VAD Implementation (ONNX-based)
// ============================================================================

/// Silero VAD V5 implementation using ONNX Runtime
///
/// Model: https://github.com/snakers4/silero-vad
/// Expects 16kHz mono audio, 256 samples per chunk (16ms).
/// See docs/silero-vad-v5.md for details.
#[allow(dead_code)]
pub struct SileroVad {
    session: Session,
    /// Combined LSTM state [2, 1, 128] (Silero VAD v5 uses a single state tensor)
    state: Array3<f32>,
    /// Sample rate tensor (scalar)
    sample_rate: Array1<i64>,
}

#[allow(dead_code)]
impl SileroVad {
    /// Path to bundled Silero VAD model
    const MODEL_RESOURCE: &'static [u8] = include_bytes!("../resources/silero_vad.onnx");

    /// Create new Silero VAD instance
    ///
    /// Uses catch_unwind to prevent ONNX Runtime panics (e.g. from ABI
    /// incompatibility on macOS < 13.3) from poisoning mutexes or crashing
    /// the Tauri IPC layer.
    pub fn new() -> Result<Self> {
        // Early check: skip if ORT is known to be unavailable
        if !crate::is_onnx_runtime_available() {
            return Err(anyhow::anyhow!(
                "ONNX Runtime is not available on this system. \
                 VAD requires ONNX Runtime 1.23+ which needs macOS 13.3+."
            ));
        }

        // Wrap session creation in catch_unwind to prevent panics from
        // cascading through the Tauri async runtime.
        //
        // Note: ort rc.12 changed SessionBuilder errors to carry the builder for
        // recovery (ort::Error<SessionBuilder>), which breaks the old and_then
        // chain because each step's error type is different. Build step-by-step
        // and convert errors to strings to sidestep the generic parameter.
        let session_result =
            std::panic::catch_unwind(|| -> std::result::Result<Session, String> {
                let b = Session::builder().map_err(|e| e.to_string())?;
                // commit_from_memory takes &mut self in rc.12
                let mut b = b
                    .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
                    .map_err(|e| e.to_string())?;
                b.commit_from_memory(Self::MODEL_RESOURCE)
                    .map_err(|e| e.to_string())
            });

        let session = match session_result {
            Ok(Ok(session)) => session,
            Ok(Err(e)) => return Err(anyhow::anyhow!("Failed to create VAD session: {}", e)),
            Err(panic_payload) => {
                let msg = panic_payload
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                log::error!(
                    "[VAD] ONNX Runtime panicked during session creation: {}",
                    msg
                );
                return Err(anyhow::anyhow!(
                    "ONNX Runtime crashed during VAD initialization ({}). \
                     This typically means ONNX Runtime is incompatible with this OS version.",
                    msg
                ));
            }
        };

        Ok(Self {
            session,
            state: Array3::zeros((2, 1, 128)),
            sample_rate: Array1::from_vec(vec![16000i64]),
        })
    }

    /// Load from custom model path
    #[allow(dead_code)]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        // ort rc.12: SessionBuilder errors carry the builder (non-Send/Sync),
        // so convert errors to anyhow at each step; commit_from_file takes &mut self.
        let b = Session::builder()
            .map_err(|e| anyhow::anyhow!("Failed to create session builder: {}", e))?;
        let mut b = b
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Failed to set optimization level: {}", e))?;
        let session = b
            .commit_from_file(path)
            .map_err(|e| anyhow::anyhow!("Failed to load VAD model: {}", e))?;

        Ok(Self {
            session,
            state: Array3::zeros((2, 1, 128)),
            sample_rate: Array1::from_vec(vec![16000i64]),
        })
    }
}

impl VadBackend for SileroVad {
    fn predict(&mut self, samples: &[f32]) -> f32 {
        // Input shape: [batch, samples] = [1, 512]
        let input = Array2::from_shape_vec((1, samples.len()), samples.to_vec())
            .expect("Invalid sample count");

        // Silero VAD v5: single "state" tensor [2, 1, 128], scalar "sr"
        let result = self.session.run(ort::inputs![
            "input" => Tensor::from_array(input).unwrap(),
            "sr" => Tensor::from_array(self.sample_rate.clone()).unwrap(),
            "state" => Tensor::from_array(self.state.clone()).unwrap(),
        ]);

        match result {
            Ok(outputs) => {
                // Update combined state for next call
                if let Ok((shape, data)) = outputs["stateN"].try_extract_tensor::<f32>() {
                    let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
                    if dims.len() == 3 {
                        self.state =
                            Array3::from_shape_vec((dims[0], dims[1], dims[2]), data.to_vec())
                                .unwrap_or_else(|_| Array3::zeros((2, 1, 128)));
                    }
                }

                // Extract probability
                if let Ok((_, data)) = outputs["output"].try_extract_tensor::<f32>() {
                    data.first().copied().unwrap_or(0.0)
                } else {
                    0.0
                }
            }
            Err(e) => {
                log::error!("[VAD] Inference error: {:?}", e);
                0.0
            }
        }
    }

    fn reset(&mut self) {
        self.state = Array3::zeros((2, 1, 128));
    }

    fn name(&self) -> &'static str {
        "Silero VAD V5"
    }
}

// ============================================================================
// VAD Test State (for live audio preview)
// ============================================================================

/// VAD test state for live audio preview
pub struct VadTestState {
    is_running: AtomicBool,
    threshold: AtomicU8, // 0-100 maps to 0.0-1.0
    /// Live capture session (echo-audio/cpal); dropping it stops the stream.
    capture: Mutex<Option<echo_audio::capture::CaptureHandle>>,
}

impl VadTestState {
    pub fn new() -> Self {
        Self {
            is_running: AtomicBool::new(false),
            threshold: AtomicU8::new(50), // Default 50%
            capture: Mutex::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    #[allow(dead_code)]
    pub fn set_running(&self, running: bool) {
        self.is_running.store(running, Ordering::SeqCst);
    }

    #[allow(dead_code)]
    pub fn get_threshold(&self) -> f32 {
        self.threshold.load(Ordering::SeqCst) as f32 / 100.0
    }

    pub fn set_threshold(&self, value: u8) {
        self.threshold.store(value.min(100), Ordering::SeqCst);
    }
}

impl Default for VadTestState {
    fn default() -> Self {
        Self::new()
    }
}

/// Payload emitted during VAD test
#[allow(dead_code)]
#[derive(Clone, serde::Serialize)]
pub struct VadLevelPayload {
    /// VAD speech probability (0.0-1.0)
    pub probability: f32,
    /// Whether current audio is above threshold
    pub is_speech: bool,
    /// Peak amplitude for visual meter (0.0-1.0)
    pub peak_amplitude: f32,
    /// RMS level for smoother visualization (0.0-1.0)
    pub rms_level: f32,
}

// ============================================================================
// VAD Test Loop (Platform-specific audio capture)
// ============================================================================

/// Start VAD test loop - captures audio and emits levels to the frontend.
///
/// Cross-platform via echo-audio/cpal (WASAPI / CoreAudio / ALSA). Each
/// 256-sample chunk (Silero v5 requirement) is scored and emitted as a
/// `vad-level` event (~60Hz) for the settings-panel live meter.
// TODO: on Linux, device enumeration (pactl names) and cpal device names can
// differ; non-default `device_id` values may not resolve. Default input works.
pub async fn start_vad_test(
    app: tauri::AppHandle,
    state: Arc<VadTestState>,
    device_id: String,
) -> Result<()> {
    use echo_audio::capture::{CaptureConfig, CaptureEvent, start_capture};
    use tauri::Emitter;

    if state.is_running() {
        return Err(anyhow::anyhow!("VAD test already running"));
    }

    // Build VAD detector using our implementation
    let mut vad = SileroVad::new().map_err(|e| anyhow::anyhow!("Failed to create VAD: {}", e))?;

    let device_name = if device_id == "default" || device_id.is_empty() {
        None
    } else {
        Some(device_id)
    };
    let (handle, events) = start_capture(CaptureConfig {
        device_name,
        chunk_samples: 256, // Silero VAD v5 chunk size (16ms at 16 kHz)
    })
    .map_err(|e| anyhow::anyhow!("Failed to start audio capture: {}", e))?;

    {
        let mut capture_guard = state.capture.lock().await;
        *capture_guard = Some(handle);
    }
    state.set_running(true);

    // Consume chunks on a blocking thread; emit levels until stopped.
    let state_clone = state.clone();
    tokio::task::spawn_blocking(move || {
        while state_clone.is_running() {
            let chunk = match events.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(CaptureEvent::Chunk(chunk)) => chunk,
                Ok(CaptureEvent::Error(e)) => {
                    log::error!("Error reading audio for VAD: {e}");
                    break;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if chunk.len() < 256 {
                continue; // tail flush on stop; not a full VAD frame
            }

            let peak = echo_audio::sample::peak(&chunk);
            let rms = echo_audio::sample::rms(&chunk);
            let probability = vad.predict(&chunk);
            let threshold = state_clone.get_threshold();

            let payload = VadLevelPayload {
                probability,
                is_speech: probability >= threshold,
                peak_amplitude: peak.min(1.0),
                rms_level: rms.min(1.0),
            };
            if let Err(e) = app.emit("vad-level", payload) {
                log::error!("[VAD] Failed to emit event: {:?}", e);
            }
        }
        state_clone.set_running(false);
    });

    Ok(())
}

/// Stop VAD test: flag the loop down and stop the capture session.
pub async fn stop_vad_test(state: Arc<VadTestState>) -> Result<()> {
    state.set_running(false);
    if let Some(handle) = state.capture.lock().await.take() {
        handle.stop();
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_state() {
        let state = VadTestState::new();
        assert!(!state.is_running());
        assert_eq!(state.get_threshold(), 0.5);

        state.set_threshold(75);
        assert_eq!(state.get_threshold(), 0.75);

        state.set_running(true);
        assert!(state.is_running());
    }
}
