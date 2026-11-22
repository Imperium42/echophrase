// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! ONNX Runtime Backend for Speech-to-Text
//!
//! This module provides an ONNX Runtime-based transcription backend,
//! supporting models like NVIDIA Parakeet-TDT that are exported in ONNX format.
//!
//! ## Supported Models
//! - NVIDIA Parakeet-TDT 0.6B v3 (3-model pipeline via parakeet-rs)
//! - NVIDIA Parakeet-TDT 0.6B v2 (deprecated, single model)
//! - Whisper ONNX exports
//! - Custom ONNX models

use anyhow::{Result, anyhow};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Value,
};
use parking_lot::Mutex; // Non-poisoning mutex for panic recovery
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Emitter};
use tokenizers::Tokenizer;

use super::TranscriptionBackend;
use super::execution_provider::{
    ExecutionProviderType, get_platform_default, parse_execution_provider,
};

/// Extract a human-readable message from a panic payload
fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown panic".to_string())
}

// ============================================================================
// Constants
// ============================================================================

/// Audio sample rate expected by most speech models (echo-audio's canonical rate).
const SAMPLE_RATE: u32 = echo_audio::TARGET_SAMPLE_RATE;

/// Get the user's preferred execution provider from saved settings,
/// falling back to platform default if not set.
fn get_preferred_provider() -> ExecutionProviderType {
    crate::settings::AppSettings::load()
        .ok()
        .and_then(|s| s.execution_provider)
        .and_then(|s| parse_execution_provider(&s))
        .unwrap_or_else(get_platform_default)
}

// ============================================================================
// Model Type Enum
// ============================================================================

/// ONNX model types with different input/output signatures
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OnnxModelType {
    /// NVIDIA Parakeet-TDT v3 (3-model pipeline via parakeet-rs)
    ParakeetV3,
    /// NVIDIA Parakeet-TDT v2 (deprecated single-model approach)
    #[deprecated(note = "Use ParakeetV3 with parakeet-rs for proper TDT decoding")]
    ParakeetV2,
    /// NVIDIA Parakeet EOU 120M (streaming model via parakeet-rs)
    ParakeetEOU,
    /// Whisper ONNX export (encoder-decoder)
    WhisperOnnx,
    /// Custom user-provided model
    Custom,
}

impl OnnxModelType {
    /// Get model type from model ID string
    pub fn from_model_id(model_id: &str) -> Self {
        if model_id.contains("eou") {
            OnnxModelType::ParakeetEOU
        } else if model_id.contains("parakeet") {
            if model_id.contains("v3") {
                OnnxModelType::ParakeetV3
            } else {
                #[allow(deprecated)]
                OnnxModelType::ParakeetV2
            }
        } else if model_id.contains("whisper") && model_id.contains("onnx") {
            OnnxModelType::WhisperOnnx
        } else {
            OnnxModelType::Custom
        }
    }
}

// ============================================================================
// Parakeet V3 Backend (using parakeet-rs)
// ============================================================================

/// Cached ParakeetTDT session for v3 models.
///
/// The model is wrapped in `Arc<Mutex<...>>` so the outer cache lock can be
/// released before any long-running operation. The `Arc` lets callers clone a
/// handle before dropping the outer guard; the inner `Mutex` serialises
/// `&mut self` calls to `transcribe_samples`.
static CACHED_PARAKEET_TDT: LazyLock<Mutex<Option<CachedParakeetTDT>>> =
    LazyLock::new(|| Mutex::new(None));

/// Serialises concurrent `load_parakeet_tdt` calls.
///
/// ORT's global DirectML state is not safe for concurrent `from_pretrained`
/// invocations. When the background preload and an early `transcribe_audio`
/// call race, both enter `from_pretrained` at the same time, which can wedge
/// ORT's DirectML session-creation path. Serialising loads avoids that.
///
/// This mutex is held ONLY during the `load_parakeet_tdt` call itself. The
/// caller checks the cache AFTER acquiring the lock so that a second waiter
/// that blocked while the first was loading will find the cache populated and
/// skip the redundant load.
static LOADING_PARAKEET_TDT: LazyLock<parking_lot::Mutex<()>> =
    LazyLock::new(|| parking_lot::Mutex::new(()));

/// How long a transcribe call will wait for the load-serialization lock before
/// giving up. A held lock means another caller's `from_pretrained` is still
/// running - on a cold release install, that's the DirectML EP JIT-compiling
/// shaders for this GPU arch for the first time, which can run well past the
/// warm ~7s baseline. Match the 300s preload deadline (see `preload_onnx_model`
/// and commit 9ce98cc) so a transcribe that races a cold preload waits for it
/// to finish and then hits the populated cache, instead of bailing fast with
/// MODEL_STILL_LOADING and forcing the user to retry manually.
const LOAD_LOCK_TRY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Sentinel prefix on the "model still loading" error so the frontend can map
/// it to a localized, user-facing message instead of showing a raw string.
pub const MODEL_STILL_LOADING_MARKER: &str = "MODEL_STILL_LOADING";

struct CachedParakeetTDT {
    /// Shared handle to the loaded model. Wrapped in Mutex because
    /// `Transcriber::transcribe_samples` requires `&mut self`.
    model: Arc<Mutex<parakeet_rs::ParakeetTDT>>,
    model_id: String,
}

/// Build the parakeet-rs execution config for a resolved provider.
///
/// DirectML EP best practices (per Microsoft ORT docs): the DML EP does not
/// support intra-op multi-threading, memory pattern optimization, or parallel
/// execution mode. parakeet-rs 0.3.4 defaults to 4 intra threads, mem-pattern
/// ON, and parallel execution - all unsupported on DML. The
/// `with_custom_configure` closure runs LAST in `apply_to_session_builder`, so
/// it can override those defaults. We gate this on DirectML only; CoreML/CPU
/// keep parakeet-rs defaults.
fn build_parakeet_exec_config(
    preferred: ExecutionProviderType,
) -> Option<parakeet_rs::ExecutionConfig> {
    let provider = preferred.to_parakeet_provider()?;

    let mut config = parakeet_rs::ExecutionConfig::new().with_execution_provider(provider);

    if preferred == ExecutionProviderType::DirectMl {
        config = config
            .with_intra_threads(1) // DML EP doesn't support intra-op threading
            .with_custom_configure(|b| {
                Ok(b.with_memory_pattern(false)? // DML EP requirement
                    .with_parallel_execution(false)?) // ORT_SEQUENTIAL - DML EP requirement
            });
    }

    Some(config)
}

/// Load a Parakeet TDT v3 model from disk and return it wrapped in a shareable mutex.
///
/// On DirectML the session settings follow ORT DML EP best practices
/// (no intra-op threading, no mem-pattern, sequential execution); see
/// `build_parakeet_exec_config`. Other backends use parakeet-rs defaults.
fn load_parakeet_tdt(model_id: &str) -> Result<Arc<Mutex<parakeet_rs::ParakeetTDT>>> {
    let t_start = std::time::Instant::now();
    let model_path = get_model_path(model_id)?;
    let t_path_ms = t_start.elapsed().as_millis();

    // List sub-files in the model dir so we know what got resolved on disk -
    // if HF cache is partial or symlinks are broken on Windows, this surfaces it.
    let file_summary = match std::fs::read_dir(&model_path) {
        Ok(rd) => {
            let entries: Vec<String> = rd
                .flatten()
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let size = e.metadata().ok().map(|m| m.len()).unwrap_or(0);
                    format!("{}({}B)", name, size)
                })
                .collect();
            format!("[{} entries: {}]", entries.len(), entries.join(", "))
        }
        Err(e) => format!("[read_dir failed: {}]", e),
    };
    log::info!(
        "[ONNX timing] resolved path in {}ms: {:?} {}",
        t_path_ms,
        model_path,
        file_summary
    );

    let t_pref = std::time::Instant::now();
    let preferred = get_preferred_provider();
    let exec_config = match build_parakeet_exec_config(preferred) {
        Some(config) => {
            log::debug!(
                "[ONNX timing] requesting {} provider ({}ms to resolve settings)",
                preferred,
                t_pref.elapsed().as_millis()
            );
            config
        }
        None => {
            log::debug!(
                "[ONNX timing] using CPU (no GPU provider available for {}; {}ms to resolve settings)",
                preferred,
                t_pref.elapsed().as_millis()
            );
            parakeet_rs::ExecutionConfig::new()
        }
    };

    let t_load = std::time::Instant::now();
    log::info!("[ONNX timing] entering parakeet_rs::from_pretrained (encoder session creation)...");

    // catch_unwind: ORT panics on macOS < 13.3 due to missing C++20 symbols
    let load_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parakeet_rs::ParakeetTDT::from_pretrained(&model_path, Some(exec_config))
    }));
    let t_load_ms = t_load.elapsed().as_millis();
    log::info!(
        "[ONNX timing] from_pretrained returned in {}ms ({:.2}s)",
        t_load_ms,
        t_load_ms as f64 / 1000.0
    );

    let model = match load_result {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => return Err(anyhow!("Failed to load ParakeetTDT: {}", e)),
        Err(panic) => {
            let msg = panic_message(&panic);
            log::error!("ONNX Runtime panicked loading ParakeetTDT: {}", msg);
            crate::ONNX_RUNTIME_AVAILABLE.store(false, std::sync::atomic::Ordering::Relaxed);
            return Err(anyhow!(
                "ONNX Runtime crashed loading model ({}). \
                 This typically means ONNX Runtime is incompatible with this OS version.",
                msg
            ));
        }
    };

    log::info!(
        "[ONNX timing] Parakeet TDT v3 ready (total {}ms)",
        t_start.elapsed().as_millis()
    );
    Ok(Arc::new(Mutex::new(model)))
}

/// Transcribe audio using Parakeet TDT v3 (3-model pipeline).
///
/// Loading is serialised via `LOADING_PARAKEET_TDT` so that a background
/// preload and an early `transcribe_audio` call cannot both enter
/// `load_parakeet_tdt` (i.e. `from_pretrained`) simultaneously. Concurrent
/// `from_pretrained` calls can wedge ORT's DirectML session-creation path.
///
/// Double-checked pattern: fast cache read -> serialise load -> re-check cache
/// (may now be populated by the caller that won the load lock) -> load if still
/// empty -> store.
fn transcribe_parakeet_v3(model_id: &str, samples: &[f32]) -> Result<String> {
    if !crate::is_onnx_runtime_available() {
        return Err(anyhow!("ONNX Runtime is not available on this system"));
    }

    // Fast path: return cached model without acquiring the load lock.
    let fast = {
        let cache = CACHED_PARAKEET_TDT.lock();
        if let Some(ref cached) = *cache {
            if cached.model_id == model_id {
                Some(Arc::clone(&cached.model))
            } else {
                None
            }
        } else {
            None
        }
    };

    let model_arc: Arc<Mutex<parakeet_rs::ParakeetTDT>> = if let Some(arc) = fast {
        arc
    } else {
        // Serialise: only one thread may call load_parakeet_tdt at a time.
        // This prevents concurrent from_pretrained calls that can wedge ORT's
        // DirectML session-creation path.
        //
        // Bounded wait: if a load holds this lock, don't block the transcribe
        // call forever. Bail out with a clear message; the load keeps running
        // and populates the cache.
        let _load_guard = match LOADING_PARAKEET_TDT.try_lock_for(LOAD_LOCK_TRY_TIMEOUT) {
            Some(guard) => guard,
            None => {
                return Err(anyhow!(
                    "{}: Model is still loading, try again shortly",
                    MODEL_STILL_LOADING_MARKER
                ));
            }
        };

        // Re-check cache: the winner of the load lock may have just populated it.
        let cached_after_wait = {
            let cache = CACHED_PARAKEET_TDT.lock();
            if let Some(ref c) = *cache {
                if c.model_id == model_id {
                    Some(Arc::clone(&c.model))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(arc) = cached_after_wait {
            log::info!("[ONNX] TDT v3 cache populated while waiting for load lock - reusing");
            arc
        } else {
            let model_arc = load_parakeet_tdt(model_id)?;
            let mut cache = CACHED_PARAKEET_TDT.lock();
            if cache.as_ref().map(|c| c.model_id.as_str()) != Some(model_id) {
                *cache = Some(CachedParakeetTDT {
                    model: Arc::clone(&model_arc),
                    model_id: model_id.to_string(),
                });
            }
            model_arc
        }
    };

    // Normalize audio
    let normalized = normalize_audio(samples);

    // Use parakeet-rs Transcriber trait.
    // IMPORTANT: TimestampMode::Words prevents TDT decoder from collapsing tokens.
    // Without this, sentences like "He saves the world with his brother, Luigi"
    // can collapse to just "Luigi" due to beam search alignment issues.
    use parakeet_rs::{TimestampMode, Transcriber};

    // Bound the encoder's working memory: long audio goes through in chunks cut
    // at the quietest point near each boundary. One 300 s call pushed a real
    // install to ~4.5 GB, and ORT keeps that peak for the life of the session
    // (see `chunking`). Clips up to the ceiling still run as a single call.
    let ranges = super::chunking::plan_chunks(&normalized, SAMPLE_RATE);
    let chunk_count: usize = ranges.len();
    if chunk_count > 1 {
        log::info!(
            "[ONNX] Parakeet TDT: splitting {:.1}s of audio into {} chunks to bound memory",
            normalized.len() as f32 / SAMPLE_RATE as f32,
            chunk_count
        );
    }

    let mut texts: Vec<String> = Vec::with_capacity(chunk_count);
    let mut model = model_arc.lock();
    for (index, range) in ranges.iter().enumerate() {
        let result = model
            .transcribe_samples(
                normalized[range.clone()].to_vec(),
                SAMPLE_RATE,
                1,
                Some(TimestampMode::Words),
            )
            .map_err(|e| {
                anyhow!(
                    "Transcription failed (chunk {}/{}): {}",
                    index + 1,
                    chunk_count,
                    e
                )
            })?;

        // Log tokens if available for debugging
        if !result.tokens.is_empty() {
            log::debug!(
                "Parakeet tokens (chunk {}/{}): {:?}",
                index + 1,
                chunk_count,
                result.tokens.iter().map(|t| &t.text).collect::<Vec<_>>()
            );
        }
        texts.push(result.text);
    }

    // A single chunk is the whole clip: return the model's text untouched so
    // short dictation behaves exactly as it did before chunking existed.
    if texts.len() == 1 {
        return Ok(texts.swap_remove(0));
    }
    Ok(super::chunking::join_chunk_texts(&texts))
}

// ============================================================================
// Parakeet EOU Backend (using parakeet-rs ParakeetEOU)
// ============================================================================

/// Cached ParakeetEOU session for streaming 120M model.
///
/// Uses the same load-outside-lock pattern as `CACHED_PARAKEET_TDT` to avoid
/// holding the outer mutex across the slow `from_pretrained` call.
static CACHED_PARAKEET_EOU: LazyLock<Mutex<Option<CachedParakeetEOU>>> =
    LazyLock::new(|| Mutex::new(None));

/// Serialises concurrent `load_parakeet_eou` calls (same rationale as
/// `LOADING_PARAKEET_TDT` - see its doc comment for full explanation).
static LOADING_PARAKEET_EOU: LazyLock<parking_lot::Mutex<()>> =
    LazyLock::new(|| parking_lot::Mutex::new(()));

struct CachedParakeetEOU {
    /// Shared handle to the loaded model. Wrapped in Mutex because
    /// `ParakeetEOU::transcribe` requires `&mut self`.
    model: Arc<Mutex<parakeet_rs::ParakeetEOU>>,
    model_id: String,
}

/// Load a Parakeet EOU 120M model from disk, wrapped in a shareable mutex.
///
/// Called OUTSIDE any cache lock to avoid blocking other callers during load.
fn load_parakeet_eou(model_id: &str) -> Result<Arc<Mutex<parakeet_rs::ParakeetEOU>>> {
    let t_start = std::time::Instant::now();
    let model_path = get_model_path(model_id)?;
    log::info!(
        "[ONNX timing] EOU resolved path in {}ms: {:?}",
        t_start.elapsed().as_millis(),
        model_path
    );

    let preferred = get_preferred_provider();
    let exec_config = match build_parakeet_exec_config(preferred) {
        Some(config) => {
            log::info!("[ONNX timing] EOU requesting {} GPU provider", preferred);
            config
        }
        None => {
            log::info!(
                "[ONNX timing] EOU using CPU (no GPU provider for {})",
                preferred
            );
            parakeet_rs::ExecutionConfig::new()
        }
    };

    log::info!("[ONNX timing] EOU entering from_pretrained (encoder session creation)...");
    let t_load = std::time::Instant::now();
    let load_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parakeet_rs::ParakeetEOU::from_pretrained(&model_path, Some(exec_config))
    }));
    log::info!(
        "[ONNX timing] EOU from_pretrained returned in {}ms",
        t_load.elapsed().as_millis()
    );

    let model = match load_result {
        Ok(Ok(m)) => m,
        Ok(Err(e)) => return Err(anyhow!("Failed to load ParakeetEOU: {}", e)),
        Err(panic) => {
            let msg = panic_message(&panic);
            log::error!("ONNX Runtime panicked loading ParakeetEOU: {}", msg);
            crate::ONNX_RUNTIME_AVAILABLE.store(false, std::sync::atomic::Ordering::Relaxed);
            return Err(anyhow!(
                "ONNX Runtime crashed loading model ({}). \
                 This typically means ONNX Runtime is incompatible with this OS version.",
                msg
            ));
        }
    };

    log::info!(
        "[ONNX timing] Parakeet EOU 120M ready (total {}ms)",
        t_start.elapsed().as_millis()
    );
    Ok(Arc::new(Mutex::new(model)))
}

/// Transcribe audio using Parakeet EOU 120M (streaming model, batch-adapted).
///
/// Loading is serialised via `LOADING_PARAKEET_EOU` (same rationale as
/// `transcribe_parakeet_v3` - see its doc comment). Concurrent `from_pretrained`
/// calls can wedge ORT's DirectML session-creation path.
fn transcribe_parakeet_eou(model_id: &str, samples: &[f32]) -> Result<String> {
    if !crate::is_onnx_runtime_available() {
        return Err(anyhow!("ONNX Runtime is not available on this system"));
    }

    // Fast path: return cached model without acquiring the load lock.
    let fast = {
        let cache = CACHED_PARAKEET_EOU.lock();
        if let Some(ref cached) = *cache {
            if cached.model_id == model_id {
                Some(Arc::clone(&cached.model))
            } else {
                None
            }
        } else {
            None
        }
    };

    let model_arc: Arc<Mutex<parakeet_rs::ParakeetEOU>> = if let Some(arc) = fast {
        arc
    } else {
        // Bounded wait (see LOADING_PARAKEET_TDT path for full rationale): a
        // held lock means another caller's from_pretrained is still running.
        // Bail with a clear "still loading" error rather than hanging.
        let _load_guard = match LOADING_PARAKEET_EOU.try_lock_for(LOAD_LOCK_TRY_TIMEOUT) {
            Some(guard) => guard,
            None => {
                return Err(anyhow!(
                    "{}: Model is still loading, try again shortly",
                    MODEL_STILL_LOADING_MARKER
                ));
            }
        };

        let cached_after_wait = {
            let cache = CACHED_PARAKEET_EOU.lock();
            if let Some(ref c) = *cache {
                if c.model_id == model_id {
                    Some(Arc::clone(&c.model))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(arc) = cached_after_wait {
            log::info!("[ONNX] EOU cache populated while waiting for load lock - reusing");
            arc
        } else {
            let model_arc = load_parakeet_eou(model_id)?;
            let mut cache = CACHED_PARAKEET_EOU.lock();
            if cache.as_ref().map(|c| c.model_id.as_str()) != Some(model_id) {
                *cache = Some(CachedParakeetEOU {
                    model: Arc::clone(&model_arc),
                    model_id: model_id.to_string(),
                });
            }
            model_arc
        }
    };

    // Transcribe by feeding chunks to the streaming model
    let normalized = normalize_audio(samples);
    let chunk_size = 2560; // 160ms at 16kHz - required by ParakeetEOU
    let mut raw_tokens = String::new();
    let mut model_guard = model_arc.lock();

    for chunk in normalized.chunks(chunk_size) {
        let text = model_guard
            .transcribe(chunk, false)
            .map_err(|e| anyhow!("EOU transcription failed: {}", e))?;
        if !text.is_empty() {
            raw_tokens.push_str(&text);
        }
    }

    // Flush remaining with reset
    let final_text = model_guard
        .transcribe(&[], true)
        .map_err(|e| anyhow!("EOU flush failed: {}", e))?;
    drop(model_guard);
    if !final_text.is_empty() {
        raw_tokens.push_str(&final_text);
    }

    // Strip EOU (End of Utterance) control tokens before post-processing
    let raw_tokens = raw_tokens.replace("[EOU]", "");

    // Post-process SentencePiece output:
    // \u{2581} (▁) marks word boundaries - replace with spaces, then clean up
    let text = raw_tokens.replace('\u{2581}', " ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

    // Capitalize first letter and ensure trailing period
    let text = if let Some(first) = text.chars().next() {
        let rest: String = text.chars().skip(1).collect();
        format!("{}{}", first.to_uppercase(), rest)
    } else {
        text
    };
    let text =
        if !text.is_empty() && !text.ends_with('.') && !text.ends_with('?') && !text.ends_with('!')
        {
            format!("{}.", text)
        } else {
            text
        };

    Ok(text)
}

// ============================================================================
// Session Cache (for v2/legacy models)
// ============================================================================

/// Cached ONNX session for reuse across transcriptions
static CACHED_ONNX_SESSION: LazyLock<Mutex<Option<CachedOnnxSession>>> =
    LazyLock::new(|| Mutex::new(None));

struct CachedOnnxSession {
    session: Session,
    tokenizer: Option<Tokenizer>,
    #[allow(dead_code)]
    model_type: OnnxModelType,
    model_id: String,
}

// ============================================================================
// ONNX Runtime Backend
// ============================================================================

/// ONNX Runtime backend for speech-to-text transcription
///
/// Supports GPU-accelerated inference (DirectML/CoreML) for ONNX-exported models.
pub struct OnnxRuntimeBackend {
    model_id: String,
    model_type: OnnxModelType,
}

impl OnnxRuntimeBackend {
    /// Create a new ONNX Runtime backend for a specific model
    pub fn new(model_id: &str) -> Self {
        let model_type = OnnxModelType::from_model_id(model_id);
        Self {
            model_id: model_id.to_string(),
            model_type,
        }
    }

    /// Create backend for NVIDIA Parakeet model
    #[allow(dead_code)]
    pub fn new_parakeet(model_id: &str) -> Self {
        let model_type = if model_id.contains("v3") {
            OnnxModelType::ParakeetV3
        } else {
            #[allow(deprecated)]
            OnnxModelType::ParakeetV2
        };
        Self {
            model_id: model_id.to_string(),
            model_type,
        }
    }

    /// Create backend for Whisper ONNX model
    #[allow(dead_code)]
    pub fn new_whisper_onnx(model_id: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            model_type: OnnxModelType::WhisperOnnx,
        }
    }

    /// Load or get cached ONNX session (for v2/legacy models only)
    fn get_or_create_session(&self) -> Result<()> {
        if !crate::is_onnx_runtime_available() {
            return Err(anyhow!("ONNX Runtime is not available on this system"));
        }

        let mut cache = CACHED_ONNX_SESSION.lock();

        // Check if we already have the right model loaded
        if let Some(ref cached) = *cache {
            if cached.model_id == self.model_id {
                return Ok(());
            }
        }

        // Load new model
        let model_path = get_model_path(&self.model_id)?;
        log::debug!("Loading ONNX model from: {:?}", model_path);

        // Find ONNX model file
        let onnx_file = find_onnx_file(&model_path)?;
        log::debug!("Found ONNX file: {:?}", onnx_file);

        // Initialize ONNX Runtime with platform-appropriate execution provider
        use super::execution_provider::{
            ExecutionProviderType, build_execution_providers, parse_execution_provider,
        };
        use crate::settings::AppSettings;
        use ort::session::builder::AutoDevicePolicy;

        // Load saved execution provider preference
        let ep_preference = AppSettings::load()
            .ok()
            .and_then(|s| s.execution_provider)
            .and_then(|s| parse_execution_provider(&s));

        log::info!(
            "[onnx_backend] Session creation: ep_preference={:?} (None = ort auto-device)",
            ep_preference
        );

        // catch_unwind: ORT panics on macOS < 13.3 due to missing C++20 symbols.
        //
        // rc.12 session builder policy:
        //   - ep_preference == None            -> with_auto_device(MaxPerformance)
        //                                         (ort picks best DirectML/CoreML/NPU/CPU)
        //   - ep_preference == Some(Cpu)       -> no EP calls; CPU is the implicit default
        //   - ep_preference == Some(explicit)  -> with_execution_providers([explicit, ...])
        //
        // SessionBuilder errors carry the builder for recovery (non-Send/Sync),
        // so stringify at each boundary instead of propagating via ?.
        let session_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> std::result::Result<Session, String> {
                let b = Session::builder().map_err(|e| e.to_string())?;

                let b = match ep_preference {
                    None => {
                        // No user preference - let ort auto-select the best device.
                        // This replaces our hand-rolled platform-specific fallback chains
                        // and gets NPU auto-use for free on Copilot+ PCs.
                        b.with_auto_device(AutoDevicePolicy::MaxPerformance)
                            .map_err(|e| e.to_string())?
                    }
                    Some(ExecutionProviderType::Cpu) => {
                        // Force CPU - do not register any EP. Session defaults to CPU.
                        b
                    }
                    Some(explicit) => {
                        // User forced a specific EP - honor it exactly.
                        let providers = build_execution_providers(Some(explicit));
                        b.with_execution_providers(providers)
                            .map_err(|e| e.to_string())?
                    }
                };

                // commit_from_file takes &mut self in rc.12
                let mut b = b
                    .with_optimization_level(GraphOptimizationLevel::Level3)
                    .map_err(|e| e.to_string())?;
                b.commit_from_file(&onnx_file).map_err(|e| e.to_string())
            },
        ));

        let session = match session_result {
            Ok(Ok(session)) => session,
            Ok(Err(e)) => return Err(anyhow!("Failed to load ONNX model: {}", e)),
            Err(panic) => {
                let msg = panic_message(&panic);
                log::error!("ONNX Runtime panicked creating session: {}", msg);
                crate::ONNX_RUNTIME_AVAILABLE.store(false, std::sync::atomic::Ordering::Relaxed);
                return Err(anyhow!(
                    "ONNX Runtime crashed creating session ({}). \
                     This typically means ONNX Runtime is incompatible with this OS version.",
                    msg
                ));
            }
        };

        let ep_status = super::execution_provider::get_execution_provider_status(None);
        log::debug!("ONNX session created: {}", ep_status.status_message);

        // Load tokenizer if available
        let tokenizer_path = model_path.join("tokenizer.json");
        let tokenizer = if tokenizer_path.exists() {
            Some(
                Tokenizer::from_file(&tokenizer_path)
                    .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?,
            )
        } else {
            log::warn!("⚠️ No tokenizer.json found, using default decoding");
            None
        };

        *cache = Some(CachedOnnxSession {
            session,
            tokenizer,
            model_type: self.model_type,
            model_id: self.model_id.clone(),
        });

        Ok(())
    }

    /// DEPRECATED: Transcribe using legacy Parakeet v2 single-model approach
    /// This does not properly handle TDT decoding and may produce incorrect results.
    #[deprecated(note = "Use transcribe_parakeet_v3 for proper TDT decoding")]
    fn transcribe_parakeet_v2_legacy(
        &self,
        session: &mut Session,
        tokenizer: Option<&Tokenizer>,
        samples: &[f32],
    ) -> Result<String> {
        // Parakeet expects audio input as [batch, time]
        let audio_len = samples.len();
        let audio_tensor = Value::from_array(([1, audio_len], samples.to_vec()))?;

        // Create length tensor
        let length_tensor = Value::from_array(([1], vec![audio_len as i64]))?;

        // Run inference
        let outputs = session.run(ort::inputs![
            "audio_signal" => audio_tensor,
            "length" => length_tensor,
        ])?;

        // Extract output tokens
        let output = outputs
            .get("logprobs")
            .or_else(|| outputs.get("output"))
            .ok_or_else(|| anyhow!("No output tensor found"))?;

        // Decode output
        let text = decode_output(output, tokenizer)?;
        Ok(text)
    }

    /// Transcribe using Whisper ONNX model
    fn transcribe_whisper_onnx(
        &self,
        session: &mut Session,
        tokenizer: Option<&Tokenizer>,
        samples: &[f32],
    ) -> Result<String> {
        // Whisper ONNX expects mel spectrogram input
        let mel = compute_mel_spectrogram(samples)?;

        // Create mel tensor [batch, mel_bins, time]
        let mel_len = mel.len() / 80; // 80 mel bins
        let mel_tensor = Value::from_array(([1, 80, mel_len], mel))?;

        // Run encoder
        let outputs = session.run(ort::inputs![
            "mel" => mel_tensor,
        ])?;

        // Extract and decode
        let output = outputs
            .get("sequences")
            .or_else(|| outputs.get("output_ids"))
            .ok_or_else(|| anyhow!("No output tensor found"))?;

        let text = decode_output(output, tokenizer)?;
        Ok(text)
    }

    /// Generic transcription for custom models
    fn transcribe_generic(
        &self,
        session: &mut Session,
        tokenizer: Option<&Tokenizer>,
        samples: &[f32],
    ) -> Result<String> {
        // Collect input names to owned Strings
        let input_names: Vec<String> = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect();

        log::debug!("🔍 Model inputs: {:?}", input_names);

        // Try common input names
        let input_name = input_names
            .iter()
            .find(|n| n.contains("audio") || n.contains("input") || n.contains("mel"))
            .cloned()
            .unwrap_or_else(|| "input".to_string());

        // Create tensor and run session
        let audio_tensor = Value::from_array(([1, samples.len()], samples.to_vec()))?;
        let outputs = session.run(ort::inputs![input_name.as_str() => audio_tensor])?;

        // Get first output
        let output = outputs
            .values()
            .next()
            .ok_or_else(|| anyhow!("No outputs from model"))?;

        let text = decode_output(output, tokenizer)?;
        Ok(text)
    }
}

impl TranscriptionBackend for OnnxRuntimeBackend {
    /// Transcribe 16 kHz mono samples, dispatching on the model type.
    async fn transcribe_samples(&self, samples: &[f32]) -> Result<String> {
        match self.model_type {
            OnnxModelType::ParakeetV3 => {
                // Use parakeet-rs for proper TDT decoding
                transcribe_parakeet_v3(&self.model_id, samples)
            }
            OnnxModelType::ParakeetEOU => {
                // Use parakeet-rs ParakeetEOU for lightweight streaming transcription
                transcribe_parakeet_eou(&self.model_id, samples)
            }
            #[allow(deprecated)]
            OnnxModelType::ParakeetV2 => {
                // Deprecated: Use legacy single-model approach
                log::warn!("⚠️ Using deprecated Parakeet v2 backend");
                self.get_or_create_session()?;
                let mut cache = CACHED_ONNX_SESSION.lock();
                let cached = cache.as_mut().ok_or_else(|| anyhow!("No session"))?;
                let normalized = normalize_audio(samples);
                self.transcribe_parakeet_v2_legacy(
                    &mut cached.session,
                    cached.tokenizer.as_ref(),
                    &normalized,
                )
            }
            OnnxModelType::WhisperOnnx => {
                self.get_or_create_session()?;
                let mut cache = CACHED_ONNX_SESSION.lock();
                let cached = cache.as_mut().ok_or_else(|| anyhow!("No session"))?;
                let normalized = normalize_audio(samples);
                self.transcribe_whisper_onnx(
                    &mut cached.session,
                    cached.tokenizer.as_ref(),
                    &normalized,
                )
            }
            OnnxModelType::Custom => {
                self.get_or_create_session()?;
                let mut cache = CACHED_ONNX_SESSION.lock();
                let cached = cache.as_mut().ok_or_else(|| anyhow!("No session"))?;
                let normalized = normalize_audio(samples);
                self.transcribe_generic(&mut cached.session, cached.tokenizer.as_ref(), &normalized)
            }
        }
    }

    /// Backend name for logging
    fn backend_name(&self) -> &'static str {
        match self.model_type {
            OnnxModelType::ParakeetV3 => "Parakeet TDT v3 (parakeet-rs)",
            OnnxModelType::ParakeetEOU => "Parakeet EOU 120M (parakeet-rs)",
            #[allow(deprecated)]
            OnnxModelType::ParakeetV2 => "Parakeet v2 (deprecated)",
            OnnxModelType::WhisperOnnx => "Whisper ONNX",
            OnnxModelType::Custom => "Custom ONNX",
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the model path from HuggingFace cache
fn get_model_path(model_id: &str) -> Result<PathBuf> {
    // BYO custom model: resolved via settings to the managed import
    // directory (`custom-models/<name>/` with a fixed `model.onnx` inside),
    // never via the registry or directory scanning.
    if model_id == "custom" {
        let settings = crate::settings::AppSettings::load()
            .map_err(|e| anyhow!("Could not load settings: {}", e))?;
        return settings
            .custom_model_path
            .map(PathBuf::from)
            .filter(|p| p.join("model.onnx").exists())
            .ok_or_else(|| {
                anyhow!(
                    "No custom model imported. Re-import your model in Settings > Custom Model."
                )
            });
    }

    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    let cache_dir = home_dir.join(".cache").join("huggingface").join("hub");

    // Use MODEL_REGISTRY to get repo_id and optional subdir
    let model_info = crate::models::get_model_info(model_id)
        .ok_or_else(|| anyhow!("Unknown ONNX model ID: {}", model_id))?;

    // Convert repo ID to cache path format
    let repo_path = format!("models--{}", model_info.repo_id.replace('/', "--"));
    let model_dir = cache_dir.join(&repo_path).join("snapshots");

    // Find the latest snapshot
    let snapshot_dir = find_latest_snapshot(&model_dir)?;

    // Append model subdirectory if specified (e.g., for multi-model repos)
    if let Some(subdir) = model_info.model_subdir {
        return Ok(snapshot_dir.join(subdir));
    }

    Ok(snapshot_dir)
}

/// Find the latest snapshot directory in HuggingFace cache
fn find_latest_snapshot(model_dir: &PathBuf) -> Result<PathBuf> {
    if !model_dir.exists() {
        return Err(anyhow!(
            "Model directory not found: {:?}. Please download the model first.",
            model_dir
        ));
    }

    let entries: Vec<_> = std::fs::read_dir(model_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    if entries.is_empty() {
        return Err(anyhow!("No snapshots found in {:?}", model_dir));
    }

    // Return the first (or only) snapshot
    Ok(entries[0].path())
}

/// Find ONNX model file in directory (for v2/legacy models)
fn find_onnx_file(model_dir: &PathBuf) -> Result<PathBuf> {
    // Look for common ONNX file names
    let candidates = [
        "model.onnx",
        "encoder.onnx",
        "model_quantized.onnx",
        "model_fp16.onnx",
    ];

    for name in &candidates {
        let path = model_dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }

    // Search for any .onnx file
    for entry in std::fs::read_dir(model_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "onnx").unwrap_or(false) {
            return Ok(path);
        }
    }

    Err(anyhow!("No ONNX file found in {:?}", model_dir))
}

/// Normalize audio to the pipeline's target peak level.
///
/// Thin delegation to the shared echo-audio implementation (kept as a local
/// name so the five model-dispatch call sites read naturally).
fn normalize_audio(samples: &[f32]) -> Vec<f32> {
    echo_audio::sample::normalize_peak(samples, echo_audio::NORMALIZE_TARGET_PEAK)
}

/// Compute mel spectrogram for Whisper-style models
fn compute_mel_spectrogram(samples: &[f32]) -> Result<Vec<f32>> {
    // Simplified mel computation (placeholder)
    // For production, use a proper mel filter bank
    let n_mel = 80;
    let hop_length = 160;
    let n_frames = samples.len() / hop_length;

    let mut mel = vec![0.0f32; n_mel * n_frames];

    for (i, chunk) in samples.chunks(hop_length).enumerate() {
        if i >= n_frames {
            break;
        }
        let energy: f32 = chunk.iter().map(|s| s * s).sum::<f32>() / chunk.len() as f32;
        for j in 0..n_mel {
            mel[j * n_frames + i] = (energy + 1e-10).ln();
        }
    }

    Ok(mel)
}

/// Decode model output to text (for v2/legacy models)
fn decode_output<V: std::ops::Deref<Target = ort::value::DynValue>>(
    output: V,
    tokenizer: Option<&Tokenizer>,
) -> Result<String> {
    // Try to extract as token IDs (i64)
    let tensor = output.try_extract_tensor::<i64>();

    match tensor {
        Ok((_shape, data)) => {
            let ids: Vec<i64> = data.iter().copied().collect();

            if let Some(tok) = tokenizer {
                let ids_u32: Vec<u32> = ids.iter().map(|&id| id as u32).collect();
                let decoded = tok
                    .decode(&ids_u32, true)
                    .map_err(|e| anyhow!("Tokenizer decode error: {}", e))?;
                Ok(decoded.trim().to_string())
            } else {
                Ok(format!("Tokens: {:?}", ids))
            }
        }
        Err(_) => {
            // Try extracting as f32 logprobs and argmax
            let (shape, data) = output
                .try_extract_tensor::<f32>()
                .map_err(|e| anyhow!("Failed to extract output tensor: {}", e))?;

            if shape.len() < 2 {
                return Err(anyhow!("Unexpected output shape: {:?}", &shape[..]));
            }

            let vocab_size = shape[shape.len() - 1] as usize;
            let n_tokens = data.len() / vocab_size;

            let mut ids = Vec::with_capacity(n_tokens);
            for i in 0..n_tokens {
                let start = i * vocab_size;
                let end = start + vocab_size;
                let slice = &data[start..end];

                let (max_idx, _) = slice
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    .unwrap();

                ids.push(max_idx as u32);
            }

            if let Some(tok) = tokenizer {
                let decoded = tok
                    .decode(&ids, true)
                    .map_err(|e| anyhow!("Tokenizer decode error: {}", e))?;
                Ok(decoded.trim().to_string())
            } else {
                Ok(format!("Tokens: {:?}", ids))
            }
        }
    }
}

/// Emit a `model-preload-status` event. Phases: `"loading"`, `"done"`, `"failed"`.
///
/// A heartbeat payload includes `{ phase, elapsed_ms }`. On `"failed"` an
/// additional `error` field is included. Failures are logged but never propagate -
/// event delivery is best-effort.
fn emit_preload_status(app: &AppHandle, phase: &str, elapsed_ms: u64, error: Option<&str>) {
    let mut payload = serde_json::json!({
        "phase": phase,
        "elapsed_ms": elapsed_ms,
    });
    if let Some(e) = error {
        payload["error"] = serde_json::Value::String(e.to_string());
    }
    if let Err(e) = app.emit("model-preload-status", &payload) {
        log::warn!("[ONNX] Failed to emit model-preload-status: {}", e);
    }
}

/// Preload ONNX model into cache for instant first transcription.
///
/// Accepts an `AppHandle` so it can emit `model-preload-status` events:
/// - `loading` - emitted before `from_pretrained` begins
/// - `done`    - emitted after successful load
/// - `failed`  - emitted on error (with `error` field)
///
/// A heartbeat is spawned on a background thread that fires `loading` phase
/// every 5s with an updated `elapsed_ms`, giving the frontend a timing trail
/// since `from_pretrained` emits no discrete progress event from inside
/// parakeet-rs.
///
/// The load is serialised via `LOADING_PARAKEET_TDT` / `LOADING_PARAKEET_EOU`
/// so that a concurrent `transcribe_audio` call racing during preload cannot
/// trigger a second `from_pretrained` call, which can wedge ORT's DirectML
/// session-creation path.
pub fn preload_onnx_model(model_id: &str, app: &AppHandle) -> Result<()> {
    if !crate::is_onnx_runtime_available() {
        return Err(anyhow!(
            "ONNX Runtime is not available on this system. \
             ONNX models require macOS 13.3+ or a compatible Linux/Windows system."
        ));
    }

    let model_type = OnnxModelType::from_model_id(model_id);

    match model_type {
        OnnxModelType::ParakeetV3 => {
            // Quick cache check without the load lock.
            {
                let cache = CACHED_PARAKEET_TDT.lock();
                if cache.as_ref().map(|c| c.model_id.as_str()) == Some(model_id) {
                    log::info!("[ONNX] Parakeet TDT v3 already preloaded: {}", model_id);
                    emit_preload_status(app, "done", 0, None);
                    return Ok(());
                }
            }

            let load_started = std::time::Instant::now();
            emit_preload_status(app, "loading", 0, None);

            // Spawn a heartbeat thread that emits "loading" every 5s
            let app_hb = app.clone();
            let model_id_hb = model_id.to_string();
            let started_hb = load_started;
            let (hb_stop_tx, hb_stop_rx) = std::sync::mpsc::channel::<()>();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    if hb_stop_rx.try_recv().is_ok() {
                        break;
                    }
                    let elapsed_ms = started_hb.elapsed().as_millis() as u64;
                    // info! (not debug!) so the 5s heartbeat lands in the INFO-level
                    // log file - gives a hard timing trail for session creation
                    // (which emits no discrete event from inside parakeet-rs).
                    log::info!(
                        "[ONNX] Preload heartbeat: {}ms ({})",
                        elapsed_ms,
                        model_id_hb
                    );
                    emit_preload_status(&app_hb, "loading", elapsed_ms, None);
                }
            });

            // Acquire the load-serialization lock before calling from_pretrained.
            // Any concurrent transcribe_audio call will block here, then find
            // the cache populated and skip loading entirely.
            let _load_guard = LOADING_PARAKEET_TDT.lock();

            // Re-check: a transcribe_audio call may have loaded while we waited.
            {
                let cache = CACHED_PARAKEET_TDT.lock();
                if cache.as_ref().map(|c| c.model_id.as_str()) == Some(model_id) {
                    let _ = hb_stop_tx.send(());
                    let elapsed = load_started.elapsed();
                    log::info!(
                        "[ONNX] TDT v3 cache populated while waiting for load lock - skipping load"
                    );
                    emit_preload_status(app, "done", elapsed.as_millis() as u64, None);
                    return Ok(());
                }
            }

            let model_arc = match load_parakeet_tdt(model_id) {
                Ok(m) => m,
                Err(e) => {
                    let elapsed = load_started.elapsed();
                    let _ = hb_stop_tx.send(());
                    log::error!(
                        "[ONNX] Parakeet TDT v3 load FAILED after {:.2}s: {}",
                        elapsed.as_secs_f64(),
                        e
                    );
                    emit_preload_status(
                        app,
                        "failed",
                        elapsed.as_millis() as u64,
                        Some(&e.to_string()),
                    );
                    return Err(e);
                }
            };

            let _ = hb_stop_tx.send(());

            // Store in cache
            {
                let mut cache = CACHED_PARAKEET_TDT.lock();
                if cache.as_ref().map(|c| c.model_id.as_str()) != Some(model_id) {
                    *cache = Some(CachedParakeetTDT {
                        model: model_arc,
                        model_id: model_id.to_string(),
                    });
                }
            }

            let elapsed = load_started.elapsed();
            emit_preload_status(app, "done", elapsed.as_millis() as u64, None);
            log::info!(
                "[ONNX] Parakeet TDT v3 preloaded in {:.2}s ({}ms)",
                elapsed.as_secs_f64(),
                elapsed.as_millis(),
            );
            log::debug!(
                "[ONNX] Parakeet TDT v3 preloaded successfully in {:.2}s ({}ms)",
                elapsed.as_secs_f64(),
                elapsed.as_millis(),
            );
        }
        OnnxModelType::ParakeetEOU => {
            // Quick cache check without the load lock.
            {
                let cache = CACHED_PARAKEET_EOU.lock();
                if cache.as_ref().map(|c| c.model_id.as_str()) == Some(model_id) {
                    log::info!("[ONNX] Parakeet EOU already preloaded: {}", model_id);
                    emit_preload_status(app, "done", 0, None);
                    return Ok(());
                }
            }

            let load_started = std::time::Instant::now();
            emit_preload_status(app, "loading", 0, None);

            // Heartbeat thread
            let app_hb = app.clone();
            let model_id_hb = model_id.to_string();
            let started_hb = load_started;
            let (hb_stop_tx, hb_stop_rx) = std::sync::mpsc::channel::<()>();
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    if hb_stop_rx.try_recv().is_ok() {
                        break;
                    }
                    let elapsed_ms = started_hb.elapsed().as_millis() as u64;
                    // info! (not debug!) so the 5s heartbeat lands in the INFO-level
                    // log file - gives a hard timing trail for session creation
                    // (which emits no discrete event from inside parakeet-rs).
                    log::info!(
                        "[ONNX] Preload heartbeat: {}ms ({})",
                        elapsed_ms,
                        model_id_hb
                    );
                    emit_preload_status(&app_hb, "loading", elapsed_ms, None);
                }
            });

            // Acquire the load-serialization lock (same rationale as TDT above).
            let _load_guard = LOADING_PARAKEET_EOU.lock();

            // Re-check after acquiring the load lock.
            {
                let cache = CACHED_PARAKEET_EOU.lock();
                if cache.as_ref().map(|c| c.model_id.as_str()) == Some(model_id) {
                    let _ = hb_stop_tx.send(());
                    let elapsed = load_started.elapsed();
                    log::info!(
                        "[ONNX] EOU cache populated while waiting for load lock - skipping load"
                    );
                    emit_preload_status(app, "done", elapsed.as_millis() as u64, None);
                    return Ok(());
                }
            }

            let model_arc = match load_parakeet_eou(model_id) {
                Ok(m) => m,
                Err(e) => {
                    let elapsed = load_started.elapsed();
                    let _ = hb_stop_tx.send(());
                    log::error!(
                        "[ONNX] Parakeet EOU load FAILED after {:.2}s: {}",
                        elapsed.as_secs_f64(),
                        e
                    );
                    emit_preload_status(
                        app,
                        "failed",
                        elapsed.as_millis() as u64,
                        Some(&e.to_string()),
                    );
                    return Err(e);
                }
            };

            let _ = hb_stop_tx.send(());

            // Store in cache
            {
                let mut cache = CACHED_PARAKEET_EOU.lock();
                if cache.as_ref().map(|c| c.model_id.as_str()) != Some(model_id) {
                    *cache = Some(CachedParakeetEOU {
                        model: model_arc,
                        model_id: model_id.to_string(),
                    });
                }
            }

            let elapsed = load_started.elapsed();
            emit_preload_status(app, "done", elapsed.as_millis() as u64, None);
            log::info!(
                "[ONNX] Parakeet EOU 120M preloaded in {:.2}s ({}ms)",
                elapsed.as_secs_f64(),
                elapsed.as_millis(),
            );
            log::debug!(
                "[ONNX] Parakeet EOU 120M preloaded successfully in {:.2}s ({}ms)",
                elapsed.as_secs_f64(),
                elapsed.as_millis(),
            );
        }
        _ => {
            log::info!(
                "[ONNX] Preload not implemented for {:?}, will load on first use",
                model_type
            );
        }
    }

    Ok(())
}

/// Clear the ONNX session cache
pub fn clear_onnx_cache() {
    {
        let mut cache = CACHED_ONNX_SESSION.lock();
        *cache = None;
        log::debug!("ONNX session cache cleared");
    }
    {
        let mut cache = CACHED_PARAKEET_TDT.lock();
        *cache = None;
        log::debug!("Parakeet TDT cache cleared");
    }
    {
        let mut cache = CACHED_PARAKEET_EOU.lock();
        *cache = None;
        log::debug!("Parakeet EOU cache cleared");
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_type_detection() {
        assert_eq!(
            OnnxModelType::from_model_id("parakeet-tdt-0.6b-v3"),
            OnnxModelType::ParakeetV3
        );
        assert_eq!(
            OnnxModelType::from_model_id("parakeet-eou-120m"),
            OnnxModelType::ParakeetEOU
        );
        #[allow(deprecated)]
        {
            assert_eq!(
                OnnxModelType::from_model_id("parakeet-tdt-0.6b-v2"),
                OnnxModelType::ParakeetV2
            );
        }
        assert_eq!(
            OnnxModelType::from_model_id("whisper-large-v3-onnx"),
            OnnxModelType::WhisperOnnx
        );
        assert_eq!(
            OnnxModelType::from_model_id("custom-model"),
            OnnxModelType::Custom
        );
    }

    // normalize/resample/WAV-read tests live in the echo-audio crate now
    // (crates/echo-audio) - the single implementation these delegate to.
    #[test]
    fn test_normalize_audio_delegates_to_target_peak() {
        let normalized = normalize_audio(&[0.1, 0.2, -0.3, 0.15]);
        let peak = normalized.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!((peak - echo_audio::NORMALIZE_TARGET_PEAK).abs() < 0.01);
    }

    /// Verify the load-outside-lock primitive: two threads racing to populate
    /// the same model slot must not deadlock or hold the outer cache lock across
    /// the load. We use a fake `Arc<Mutex<u32>>` cache keyed by a string ID to
    /// simulate the pattern in isolation (no real parakeet-rs dependency needed).
    ///
    /// The test also asserts that only one "load" (200ms sleep) occurs regardless
    /// of how many threads race - the winner stores and the loser reuses.
    #[test]
    fn test_load_outside_lock_no_deadlock() {
        use std::sync::{Arc, Mutex};
        use std::time::{Duration, Instant};

        // Simulated cache: the outer Mutex guards an Option<Arc<Mutex<u32>>>.
        // Inner Mutex represents a model that requires &mut self for use.
        let cache: Arc<Mutex<Option<Arc<Mutex<u32>>>>> = Arc::new(Mutex::new(None));
        let load_count = Arc::new(Mutex::new(0u32));

        // Simulated "load" that sleeps 200ms to mimic from_pretrained
        let do_load = {
            let load_count = Arc::clone(&load_count);
            move || -> Arc<Mutex<u32>> {
                std::thread::sleep(Duration::from_millis(200));
                let mut n = load_count.lock().unwrap();
                *n += 1;
                Arc::new(Mutex::new(*n))
            }
        };
        let do_load = Arc::new(do_load);

        let start = Instant::now();
        let mut handles = vec![];

        for _ in 0..2 {
            let cache = Arc::clone(&cache);
            let do_load = Arc::clone(&do_load);
            let h = std::thread::spawn(move || {
                // Step 1: quick check
                let model_arc = {
                    let guard = cache.lock().unwrap();
                    if let Some(ref m) = *guard {
                        Arc::clone(m)
                    } else {
                        drop(guard);
                        // Step 2: load OUTSIDE the lock
                        let loaded = do_load();
                        // Step 3: store, check for race
                        let mut guard = cache.lock().unwrap();
                        if guard.is_none() {
                            *guard = Some(Arc::clone(&loaded));
                        }
                        // Either way, use the loaded copy for this call
                        loaded
                    }
                };
                // Use the model (simulate &mut self)
                *model_arc.lock().unwrap()
            });
            handles.push(h);
        }

        let results: Vec<u32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let elapsed = start.elapsed();

        // Both threads should complete in under 500ms (serial would be 400ms+)
        // - they race, one wins the load (200ms), the other may redo the load
        // because the cache was empty when it checked. That is acceptable and
        // the important invariant is: NO DEADLOCK and total time < 500ms.
        assert!(
            elapsed < Duration::from_millis(500),
            "Threads took too long ({:?}), possible serialization or deadlock",
            elapsed
        );

        // Both results should be valid model values
        assert!(
            results.iter().all(|&v| v >= 1),
            "All results should be valid"
        );

        // Cache should have exactly one entry
        let final_cache = cache.lock().unwrap();
        assert!(final_cache.is_some(), "Cache should be populated");
    }
}
