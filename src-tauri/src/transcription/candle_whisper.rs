// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
// SPDX-FileCopyrightText: 2023 HuggingFace
//
// SPDX-License-Identifier: MPL-2.0 AND (MIT OR Apache-2.0)

// Candle Whisper backend for Echophrase
//
// Portions of this file are derived from the Candle Whisper example:
// https://github.com/huggingface/candle/blob/main/candle-examples/examples/whisper/main.rs
//
// Copyright (c) 2023 HuggingFace
// Licensed under MIT OR Apache-2.0
// https://github.com/huggingface/candle
//
// Modified for integration with Echophrase by adding:
// - Async API wrappers
// - CUDA device caching
// - Model path resolution from HuggingFace cache
// - Audio file I/O utilities

use anyhow::{Context, Result, anyhow};
use candle_core::{Device, IndexOp, Tensor};
use candle_nn::{
    VarBuilder,
    ops::{log_softmax, softmax},
};
use candle_transformers::models::whisper::{self as m, Config, audio};
use parking_lot::Mutex; // Non-poisoning mutex for panic recovery
use rand::SeedableRng;
use rand::distributions::{Distribution, weighted::WeightedIndex};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tauri::{AppHandle, Emitter};
use tokenizers::Tokenizer;

/// Payload for realtime transcription segment preview events
#[derive(Debug, Clone, Serialize)]
pub struct SegmentPreview {
    /// Accumulated transcription text so far
    pub text: String,
    /// Whether this is the final segment
    pub is_final: bool,
}

// Audio processing constants
// Normalization levels live in echo_audio (NORMALIZE_TARGET_PEAK) and
// types::AudioNormalizationConfig (the Whisper amplify-only variant).

// Decoder configuration constants
/// Random seed for decoder sampling (speed of light in m/s)
/// Using a constant seed ensures reproducible transcriptions
const DECODER_RANDOM_SEED: u64 = 299792458;

/// Default language for transcription (English)
const DEFAULT_LANGUAGE_TOKEN: &str = "<|en|>";

/// Enable/disable timestamp tokens in output
/// Set to false for short clips to get cleaner transcriptions
const ENABLE_TIMESTAMPS: bool = false;

// Model and tokenizer types
enum Model {
    Normal(m::model::Whisper),
    Quantized(m::quantized_model::Whisper),
}

impl Model {
    fn config(&self) -> &Config {
        match self {
            Self::Normal(m) => &m.config,
            Self::Quantized(m) => &m.config,
        }
    }

    fn encoder_forward(&mut self, x: &Tensor, flush: bool) -> Result<Tensor> {
        match self {
            Self::Normal(m) => m.encoder.forward(x, flush).map_err(anyhow::Error::from),
            Self::Quantized(m) => m.encoder.forward(x, flush).map_err(anyhow::Error::from),
        }
    }

    fn decoder_forward(&mut self, x: &Tensor, xa: &Tensor, flush: bool) -> Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.forward(x, xa, flush).map_err(anyhow::Error::from),
            Self::Quantized(m) => m.decoder.forward(x, xa, flush).map_err(anyhow::Error::from),
        }
    }

    fn decoder_final_linear(&self, x: &Tensor) -> Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.final_linear(x).map_err(anyhow::Error::from),
            Self::Quantized(m) => m.decoder.final_linear(x).map_err(anyhow::Error::from),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct DecodingResult {
    tokens: Vec<u32>,
    text: String,
    avg_logprob: f64,
    no_speech_prob: f64,
    temperature: f64,
    compression_ratio: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Segment {
    start: f64,
    duration: f64,
    dr: DecodingResult,
}

struct Decoder {
    model: Model,
    rng: rand::rngs::StdRng,
    task: Option<Task>,
    timestamps: bool,
    max_initial_timestamp_index: Option<u32>,
    verbose: bool,
    tokenizer: Tokenizer,
    suppress_tokens: Tensor,
    sot_token: u32,
    transcribe_token: u32,
    translate_token: u32,
    eot_token: u32,
    no_speech_token: u32,
    no_timestamps_token: u32,
    language_token: Option<u32>,
}

impl Decoder {
    #[allow(clippy::too_many_arguments)]
    fn new(
        model: Model,
        tokenizer: Tokenizer,
        seed: u64,
        device: &Device,
        language_token: Option<u32>,
        task: Option<Task>,
        timestamps: bool,
        max_initial_timestamp_index: Option<u32>,
        verbose: bool,
    ) -> Result<Self> {
        let no_timestamps_token = token_id(&tokenizer, m::NO_TIMESTAMPS_TOKEN)?;
        // Suppress the notimestamps token when in timestamps mode.
        // When timestamps=false, suppress all timestamp tokens (> no_timestamps_token).
        // https://github.com/openai/whisper/blob/e8622f9afc4eba139bf796c210f5c01081000472/whisper/decoding.py#L452
        let suppress_tokens: Vec<f32> = (0..model.config().vocab_size as u32)
            .map(|i| {
                if model.config().suppress_tokens.contains(&i)
                    || (timestamps && i == no_timestamps_token)
                    || (!timestamps && i > no_timestamps_token)
                // Suppress timestamp range when timestamps=false
                {
                    f32::NEG_INFINITY
                } else {
                    0f32
                }
            })
            .collect();

        let suppress_tokens = Tensor::new(suppress_tokens.as_slice(), device)?;
        let sot_token = token_id(&tokenizer, m::SOT_TOKEN)?;
        let transcribe_token = token_id(&tokenizer, m::TRANSCRIBE_TOKEN)?;
        let translate_token = token_id(&tokenizer, m::TRANSLATE_TOKEN)?;
        let eot_token = token_id(&tokenizer, m::EOT_TOKEN)?;
        let no_speech_token = m::NO_SPEECH_TOKENS
            .iter()
            .find_map(|token| token_id(&tokenizer, token).ok());
        let no_speech_token = match no_speech_token {
            None => anyhow::bail!("unable to find any non-speech token"),
            Some(n) => n,
        };
        Ok(Self {
            model,
            rng: rand::rngs::StdRng::seed_from_u64(seed),
            tokenizer,
            task,
            timestamps,
            max_initial_timestamp_index,
            verbose,
            suppress_tokens,
            sot_token,
            transcribe_token,
            translate_token,
            eot_token,
            no_speech_token,
            language_token,
            no_timestamps_token,
        })
    }

    fn apply_timestamp_rules(&self, input_logits: &Tensor, tokens: &[u32]) -> Result<Tensor> {
        let device = input_logits.device().clone();
        let timestamp_begin = self.no_timestamps_token + 1;
        let vocab_size = self.model.config().vocab_size as u32;

        // ========== SETUP: Extract sampled tokens for analysis ==========
        let sample_begin = if self.language_token.is_some() { 3 } else { 2 };
        let sampled_tokens = if tokens.len() > sample_begin {
            &tokens[sample_begin..]
        } else {
            &[]
        };

        let mut masks = Vec::new();
        // Pre-allocate reusable mask buffer to avoid repeated allocations
        let mut mask_buffer = vec![0.0f32; vocab_size as usize];

        // ========== RULE 1: Timestamp pairing constraints ==========
        // Timestamps must come in pairs, except directly before EOT
        if !sampled_tokens.is_empty() {
            let last_was_timestamp = sampled_tokens
                .last()
                .map(|&t| t >= timestamp_begin)
                .unwrap_or(false);

            let penultimate_was_timestamp = if sampled_tokens.len() >= 2 {
                sampled_tokens[sampled_tokens.len() - 2] >= timestamp_begin
            } else {
                false
            };

            if last_was_timestamp {
                if penultimate_was_timestamp {
                    // Has to be non-timestamp - suppress timestamp tokens
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i >= timestamp_begin {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
                } else {
                    // Cannot be normal text tokens - suppress everything before EOT
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i < self.eot_token {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
                }
            }

            // ========== RULE 2: Non-decreasing timestamp constraint ==========
            // Timestamps shouldn't decrease; forbid timestamp tokens smaller than the last
            let timestamp_tokens: Vec<u32> = sampled_tokens
                .iter()
                .filter(|&&t| t >= timestamp_begin)
                .cloned()
                .collect();

            if !timestamp_tokens.is_empty() {
                let timestamp_last = if last_was_timestamp && !penultimate_was_timestamp {
                    *timestamp_tokens.last().unwrap()
                } else {
                    timestamp_tokens.last().unwrap() + 1
                };

                for i in 0..vocab_size {
                    mask_buffer[i as usize] = if i >= timestamp_begin && i < timestamp_last {
                        f32::NEG_INFINITY
                    } else {
                        0.0
                    };
                }
                masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
            }
        }

        // ========== RULE 3: Force initial timestamp ==========
        // At the beginning, suppress generating non-timestamp tokens
        // ONLY when timestamps are enabled!
        if self.timestamps && tokens.len() == sample_begin {
            for i in 0..vocab_size {
                mask_buffer[i as usize] = if i < timestamp_begin {
                    f32::NEG_INFINITY
                } else {
                    0.0
                };
            }
            masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);

            // Apply the max_initial_timestamp constraint
            if let Some(max_initial_timestamp_index) = self.max_initial_timestamp_index {
                let last_allowed = timestamp_begin + max_initial_timestamp_index;
                if last_allowed < vocab_size {
                    for i in 0..vocab_size {
                        mask_buffer[i as usize] = if i > last_allowed {
                            f32::NEG_INFINITY
                        } else {
                            0.0
                        };
                    }
                    masks.push(Tensor::new(mask_buffer.as_slice(), &device)?);
                }
            }
        }

        // ========== APPLY MASKS: Apply all constraint masks ==========
        let mut logits = input_logits.clone();
        for mask in masks {
            logits = logits.broadcast_add(&mask)?;
        }

        // ========== RULE 4: Probability-based timestamp preference ==========
        // If sum of probability over timestamps is above any other token, sample timestamp
        let log_probs = log_softmax(&logits, 0)?;

        // Extract timestamp and text log probabilities
        let timestamp_log_probs = log_probs.narrow(
            0,
            timestamp_begin as usize,
            vocab_size as usize - timestamp_begin as usize,
        )?;

        let text_log_probs = log_probs.narrow(0, 0, timestamp_begin as usize)?;

        // Implement logsumexp for timestamp tokens (numerically stable)
        let timestamp_logprob = {
            let max_val = timestamp_log_probs.max(0)?;
            let shifted = timestamp_log_probs.broadcast_sub(&max_val)?;
            let exp_shifted = shifted.exp()?;
            let sum_exp = exp_shifted.sum(0)?;
            let log_sum = sum_exp.log()?;
            max_val.broadcast_add(&log_sum)?.to_scalar::<f32>()?
        };

        // Get max text token log probability
        let max_text_token_logprob: f32 = text_log_probs.max(0)?.to_scalar::<f32>()?;

        // Compare in log space
        if timestamp_logprob > max_text_token_logprob {
            // Only consider timestamp tokens
            for i in 0..vocab_size {
                mask_buffer[i as usize] = if i < timestamp_begin {
                    f32::NEG_INFINITY
                } else {
                    0.0
                };
            }
            let mask_tensor = Tensor::new(mask_buffer.as_slice(), &device)?;
            logits = logits.broadcast_add(&mask_tensor)?;
        }

        Ok(logits)
    }

    fn decode(&mut self, mel: &Tensor, t: f64) -> Result<DecodingResult> {
        let audio_features = self.model.encoder_forward(mel, true)?;
        if self.verbose {
            log::info!("audio features: {:?}", audio_features.dims());
        }
        let sample_len = self.model.config().max_target_positions / 2;
        let mut sum_logprob = 0f64;
        let mut no_speech_prob = f64::NAN;
        let mut tokens = vec![self.sot_token];
        if let Some(language_token) = self.language_token {
            tokens.push(language_token);
        }
        match self.task {
            None | Some(Task::Transcribe) => tokens.push(self.transcribe_token),
            Some(Task::Translate) => tokens.push(self.translate_token),
        }
        if !self.timestamps {
            tokens.push(self.no_timestamps_token);
        }
        for i in 0..sample_len {
            let tokens_t = Tensor::new(tokens.as_slice(), mel.device())?;

            // The model expects a batch dim but this inference loop does not handle
            // it so we add it at this point.
            let tokens_t = tokens_t.unsqueeze(0)?;
            let ys = self
                .model
                .decoder_forward(&tokens_t, &audio_features, i == 0)?;

            // Extract the no speech probability on the first iteration by looking at the first
            // token logits and the probability for the according token.
            if i == 0 {
                let logits = self.model.decoder_final_linear(&ys.i(..1)?)?.i(0)?.i(0)?;
                no_speech_prob = softmax(&logits, 0)?
                    .i(self.no_speech_token as usize)?
                    .to_scalar::<f32>()? as f64;
            }

            let (_, seq_len, _) = ys.dims3()?;
            let logits = self
                .model
                .decoder_final_linear(&ys.i((..1, seq_len - 1..))?)?
                .i(0)?
                .i(0)?;

            // Apply timestamp rules when timestamps are enabled
            let logits = if self.timestamps {
                self.apply_timestamp_rules(&logits, &tokens)?
            } else {
                logits
            };

            let logits = logits.broadcast_add(&self.suppress_tokens)?;

            let next_token = if t > 0f64 {
                let prs = softmax(&(&logits / t)?, 0)?;
                let logits_v: Vec<f32> = prs.to_vec1()?;
                let distr = WeightedIndex::new(&logits_v)?;
                distr.sample(&mut self.rng) as u32
            } else {
                let logits_v: Vec<f32> = logits.to_vec1()?;
                logits_v
                    .iter()
                    .enumerate()
                    .max_by(|(_, u), (_, v)| u.total_cmp(v))
                    .map(|(i, _)| i as u32)
                    .unwrap()
            };
            tokens.push(next_token);
            let prob = softmax(&logits, candle_core::D::Minus1)?
                .i(next_token as usize)?
                .to_scalar::<f32>()? as f64;
            if next_token == self.eot_token
                || tokens.len() > self.model.config().max_target_positions
            {
                break;
            }
            sum_logprob += prob.ln();
        }
        let text = self
            .tokenizer
            .decode(&tokens, true)
            .map_err(|e| anyhow!("{}", e))?;
        let avg_logprob = sum_logprob / tokens.len() as f64;

        Ok(DecodingResult {
            tokens,
            text,
            avg_logprob,
            no_speech_prob,
            temperature: t,
            compression_ratio: f64::NAN,
        })
    }

    fn decode_with_fallback(&mut self, segment: &Tensor) -> Result<DecodingResult> {
        for (i, &t) in m::TEMPERATURES.iter().enumerate() {
            let dr: Result<DecodingResult> = self.decode(segment, t);
            if i == m::TEMPERATURES.len() - 1 {
                return dr;
            }
            // On errors, we try again with a different temperature.
            match dr {
                Ok(dr) => {
                    let needs_fallback = dr.compression_ratio > m::COMPRESSION_RATIO_THRESHOLD
                        || dr.avg_logprob < m::LOGPROB_THRESHOLD;
                    if !needs_fallback || dr.no_speech_prob > m::NO_SPEECH_THRESHOLD {
                        return Ok(dr);
                    }
                }
                Err(err) => {
                    log::warn!("Error running at {t}: {err}")
                }
            }
        }
        unreachable!()
    }

    /// Extract clean text from tokens, filtering out timestamps and special tokens
    fn extract_text_from_tokens(&self, tokens: &[u32]) -> Result<String> {
        let mut clean_tokens = vec![];
        for &token in tokens {
            // Skip special tokens (SOT, EOT) and timestamp tokens
            if token == self.sot_token || token == self.eot_token {
                continue;
            }
            // Timestamp tokens are > no_timestamps_token
            if token > self.no_timestamps_token {
                continue; // Skip timestamp tokens like <|30.00|>
            }
            clean_tokens.push(token);
        }

        self.tokenizer
            .decode(&clean_tokens, true)
            .map_err(|e| anyhow!("{}", e))
    }

    fn run(&mut self, mel: &Tensor, app_handle: Option<&AppHandle>) -> Result<Vec<Segment>> {
        let (_, _, content_frames) = mel.dims3()?;
        let mut seek = 0;
        let mut segments = vec![];
        let mut accumulated_text = String::new();
        while seek < content_frames {
            let start = std::time::Instant::now();
            let time_offset = (seek * m::HOP_LENGTH) as f64 / m::SAMPLE_RATE as f64;
            let segment_size = usize::min(content_frames - seek, m::N_FRAMES);
            let mel_segment = mel.narrow(2, seek, segment_size)?;
            let segment_duration = (segment_size * m::HOP_LENGTH) as f64 / m::SAMPLE_RATE as f64;
            let dr = self.decode_with_fallback(&mel_segment)?;
            seek += segment_size;
            if dr.no_speech_prob > m::NO_SPEECH_THRESHOLD && dr.avg_logprob < m::LOGPROB_THRESHOLD {
                log::info!("no speech detected, skipping {seek} {dr:?}");
                continue;
            }
            let segment = Segment {
                start: time_offset,
                duration: segment_duration,
                dr,
            };
            if self.timestamps {
                log::info!(
                    "{:.1}s -- {:.1}s",
                    segment.start,
                    segment.start + segment.duration,
                );
                let mut tokens_to_decode = vec![];
                let mut prev_timestamp_s = 0f32;
                for &token in segment.dr.tokens.iter() {
                    if token == self.sot_token || token == self.eot_token {
                        continue;
                    }
                    // The no_timestamp_token is the last before the timestamp ones.
                    if token > self.no_timestamps_token {
                        let timestamp_s = (token - self.no_timestamps_token + 1) as f32 / 50.;
                        if !tokens_to_decode.is_empty() {
                            let text = self
                                .tokenizer
                                .decode(&tokens_to_decode, true)
                                .map_err(|e| anyhow!("{}", e))?;
                            log::info!("  {:.1}s-{:.1}s: {}", prev_timestamp_s, timestamp_s, text);
                            tokens_to_decode.clear()
                        }
                        prev_timestamp_s = timestamp_s;
                    } else {
                        tokens_to_decode.push(token)
                    }
                }
                if !tokens_to_decode.is_empty() {
                    let text = self
                        .tokenizer
                        .decode(&tokens_to_decode, true)
                        .map_err(|e| anyhow!("{}", e))?;
                    if !text.is_empty() {
                        log::info!("  {:.1}s-...: {}", prev_timestamp_s, text);
                    }
                    tokens_to_decode.clear()
                }
            } else {
                log::info!(
                    "{:.1}s -- {:.1}s: {}",
                    segment.start,
                    segment.start + segment.duration,
                    segment.dr.text,
                )
            }
            if self.verbose {
                log::info!("{seek}: {segment:?}, in {:?}", start.elapsed());
            }

            // Emit segment preview event if app handle is provided
            if let Some(app) = app_handle {
                let clean_text = self
                    .extract_text_from_tokens(&segment.dr.tokens)
                    .unwrap_or_default();
                log::info!("📢 Segment clean_text: '{}'", clean_text.trim());
                if !clean_text.trim().is_empty() {
                    if !accumulated_text.is_empty() {
                        accumulated_text.push(' ');
                    }
                    accumulated_text.push_str(clean_text.trim());

                    let preview = SegmentPreview {
                        text: accumulated_text.clone(),
                        is_final: false,
                    };
                    log::info!(
                        "📢 Emitting transcription-segment event: '{}'",
                        accumulated_text
                    );
                    match app.emit("transcription-segment", &preview) {
                        Ok(_) => log::info!("✅ Event emitted successfully"),
                        Err(e) => log::error!("❌ Failed to emit event: {}", e),
                    }
                }
            } else {
                log::info!("⚠️ No app_handle provided for preview events");
            }

            segments.push(segment)
        }
        Ok(segments)
    }

    /// Consume the decoder and return the model and tokenizer for caching
    fn into_components(self) -> (Model, Tokenizer) {
        (self.model, self.tokenizer)
    }
}

pub fn token_id(tokenizer: &Tokenizer, token: &str) -> candle_core::Result<u32> {
    match tokenizer.token_to_id(token) {
        None => candle_core::bail!("no token-id for {token}"),
        Some(id) => Ok(id),
    }
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
enum Task {
    Transcribe,
    Translate,
}

/// Check if running macOS 13 (Ventura) or later.
/// Metal's `MTLDevice.architecture` selector requires macOS 13+.
/// On older macOS, `Device::new_metal(0)` succeeds but crashes with an
/// uncatchable Objective-C exception during actual GPU computation.
#[cfg(target_os = "macos")]
fn is_macos_13_or_later() -> bool {
    use std::process::Command;
    if let Ok(output) = Command::new("sw_vers").arg("-productVersion").output() {
        if let Ok(version) = String::from_utf8(output.stdout) {
            let parts: Vec<&str> = version.trim().split('.').collect();
            if let Some(major) = parts.first().and_then(|s| s.parse::<u32>().ok()) {
                return major >= 13;
            }
        }
    }
    false
}

// Cached GPU/CPU device - initialized once
static CACHED_DEVICE: LazyLock<Result<Device, anyhow::Error>> = LazyLock::new(|| {
    #[cfg(target_os = "macos")]
    {
        if !is_macos_13_or_later() {
            log::warn!(
                "⚠️ macOS < 13 detected - using CPU (Metal API incompatible on this version)"
            );
            log::warn!("macOS < 13: Metal's architecture selector is unavailable, using CPU");
            return Ok(Device::Cpu);
        }

        match Device::new_metal(0) {
            Ok(device) => {
                log::debug!("🍎 Metal device initialized for Candle (GPU acceleration)");
                log::info!("🍎 Metal device initialized for Candle");
                Ok(device)
            }
            Err(e) => {
                log::warn!(
                    "⚠️ Metal unavailable ({}), falling back to CPU - transcription will be slow",
                    e
                );
                log::warn!("Metal unavailable: {}. Falling back to CPU.", e);
                Ok(Device::Cpu)
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(feature = "cuda")]
        {
            match Device::new_cuda(0) {
                Ok(device) => {
                    log::debug!("🎮 CUDA device initialized for Candle");
                    log::info!("🎮 CUDA device initialized for Candle backend");
                    Ok(device)
                }
                Err(e) => {
                    log::warn!("❌ Failed to initialize CUDA device: {}", e);
                    log::error!("❌ CUDA device initialization failed: {}", e);
                    Err(anyhow!("CUDA device initialization failed: {}", e))
                }
            }
        }
        #[cfg(not(feature = "cuda"))]
        {
            log::info!("Candle using CPU (build without --features cuda)");
            Ok(Device::Cpu)
        }
    }
});

// Cached model path - initialized once
static CACHED_MODEL_PATH: LazyLock<Result<PathBuf, anyhow::Error>> =
    LazyLock::new(|| match get_model_path_sync() {
        Ok(path) => {
            log::debug!(
                "📍 Cached Candle model path initialized: {}",
                path.display()
            );
            Ok(path)
        }
        Err(e) => {
            log::warn!("❌ Failed to initialize Candle model path: {}", e);
            Err(e)
        }
    });

/// Cached model components - Model + Tokenizer + Config
///
/// Uses a take-and-return ownership transfer pattern rather than holding the
/// Mutex across the entire load: `take_or_load_model` takes the Option out of
/// the guard, returns the guard immediately, and `return_model_to_cache` puts it
/// back after transcription completes. This means the Mutex is never held across
/// `load_whisper_model` (the slow path), so there is NO lock-across-load issue
/// here - no fix required (unlike the Parakeet ONNX backends in onnx_backend.rs).
static CACHED_MODEL_COMPONENTS: LazyLock<Mutex<Option<(Model, Tokenizer, Config)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Take model from cache or load if not cached
/// Returns ownership of the model components
fn take_or_load_model(device: &Device, model_path: &Path) -> Result<(Model, Tokenizer, Config)> {
    let mut guard = CACHED_MODEL_COMPONENTS.lock();

    if let Some(components) = guard.take() {
        log::debug!("⚡ [Candle] Using cached model - instant load!");
        Ok(components)
    } else {
        log::debug!("🔧 [Candle] First load - caching model for future calls...");
        load_whisper_model(model_path, device)
    }
}

/// Return model to cache for reuse
fn return_model_to_cache(model: Model, tokenizer: Tokenizer, config: Config) {
    let mut guard = CACHED_MODEL_COMPONENTS.lock();
    *guard = Some((model, tokenizer, config));
    log::debug!("💾 [Candle] Model returned to cache");
}

/// Preload model into cache without transcription
/// Call this on app startup to eliminate first-transcription delay
pub fn preload_model() -> Result<()> {
    log::debug!("🔥 [Candle] Preloading model...");

    let device = CACHED_DEVICE
        .as_ref()
        .map_err(|e| anyhow!("Device error: {}", e))?
        .clone();

    let model_path = CACHED_MODEL_PATH
        .as_ref()
        .map_err(|e| anyhow!("Model path error: {}", e))?;

    // Load model and immediately return to cache
    let (model, tokenizer, config) = take_or_load_model(&device, model_path)?;
    return_model_to_cache(model, tokenizer, config);

    log::debug!("✅ [Candle] Model preloaded successfully");
    Ok(())
}

/// Clear the model cache (for model switching)
pub fn clear_model_cache() {
    let mut guard = CACHED_MODEL_COMPONENTS.lock();
    *guard = None;
    log::debug!("🗑️ [Candle] Model cache cleared");
}

/// Main transcription function using Candle.
///
/// Thin wrapper: decode the file via the shared echo-audio crate, then run the
/// in-memory path. Normalization happens inside the buffer path
/// (AudioNormalizationConfig - amplify-only, Whisper-specific).
#[allow(dead_code)] // used by the ignored GPU integration test
pub async fn transcribe_audio_file(audio_path: &str) -> Result<String> {
    log::warn!("🎯 [Candle] Starting transcription of file: {}", audio_path);
    let samples = echo_audio::wav::read_wav_mono_16k(std::path::Path::new(audio_path))
        .with_context(|| format!("Failed to read WAV file: {}", audio_path))?;
    transcribe_audio_buffer(&samples).await
}

/// Transcribe audio from memory buffer (no file I/O)
pub async fn transcribe_audio_buffer(samples: &[f32]) -> Result<String> {
    transcribe_audio_buffer_with_preview(samples, None).await
}

/// Transcribe audio from memory buffer with optional realtime preview events
///
/// When `app_handle` is provided, emits `transcription-segment` events
/// as each segment is processed, allowing the frontend to show a
/// realtime preview of the transcription.
pub async fn transcribe_audio_buffer_with_preview(
    samples: &[f32],
    app_handle: Option<AppHandle>,
) -> Result<String> {
    let duration_secs = samples.len() as f32 / 16000.0;
    log::debug!(
        "🎤 [Candle] Processing {:.1}s of audio ({} samples)",
        duration_secs,
        samples.len()
    );

    // Get cached device
    let device = CACHED_DEVICE
        .as_ref()
        .map_err(|e| anyhow!("Failed to get device: {}", e))?
        .clone();

    // Get cached model path
    let model_path = CACHED_MODEL_PATH
        .as_ref()
        .map_err(|e| anyhow!("Failed to get model path: {}", e))?;

    let start_time = std::time::Instant::now();

    // Get model from cache or load if first time
    let (model, tokenizer, config) = take_or_load_model(&device, model_path)?;

    // Pre-decoder work: any error here must return model to cache before propagating.
    macro_rules! try_or_return_model {
        ($expr:expr, $model:ident, $tokenizer:ident, $config:ident) => {
            match $expr {
                Ok(val) => val,
                Err(e) => {
                    return_model_to_cache($model, $tokenizer, $config);
                    log::warn!("🛡️ [Candle] Model returned to cache after pre-decoder error");
                    return Err(e.into());
                }
            }
        };
    }

    // Normalize audio to optimal level for Whisper (prevents language token loops on quiet audio)
    use crate::types::AudioNormalizationConfig;
    let mut normalized_samples = samples.to_vec();
    let norm_config = AudioNormalizationConfig::default();
    norm_config.normalize(&mut normalized_samples);

    // Convert to mel spectrogram
    let mel = try_or_return_model!(
        audio_to_mel(&normalized_samples, &config, &device),
        model,
        tokenizer,
        config
    );

    // Create decoder - consumes model ownership
    let language_token = Some(try_or_return_model!(
        token_id(&tokenizer, DEFAULT_LANGUAGE_TOKEN),
        model,
        tokenizer,
        config
    ));
    let task = Some(Task::Transcribe);
    let timestamps = ENABLE_TIMESTAMPS;
    let seed = DECODER_RANDOM_SEED;
    let verbose = false;

    let mut decoder = Decoder::new(
        model,
        tokenizer,
        seed,
        &device,
        language_token,
        task,
        timestamps,
        None, // max_initial_timestamp_index
        verbose,
    )?; // If Decoder::new() fails, model was consumed - can't recover

    // Run transcription and extract text while decoder is alive.
    // Chain with and_then so any error is captured without early return.
    let transcript_result = decoder.run(&mel, app_handle.as_ref()).and_then(|segments| {
        let mut transcript_parts = vec![];
        for segment in &segments {
            let clean_text = decoder.extract_text_from_tokens(&segment.dr.tokens)?;
            if !clean_text.trim().is_empty() {
                transcript_parts.push(clean_text);
            }
        }
        Ok(transcript_parts.join(" "))
    });

    // Always recover model from decoder and return to cache
    let (model, tokenizer) = decoder.into_components();
    return_model_to_cache(model, tokenizer, config);

    let transcript = transcript_result?;

    // Emit final segment event
    if let Some(app) = &app_handle {
        let preview = SegmentPreview {
            text: transcript.clone(),
            is_final: true,
        };
        log::info!(
            "📢 Emitting FINAL transcription-segment event: '{}'",
            transcript
        );
        match app.emit("transcription-segment", &preview) {
            Ok(_) => log::info!("✅ Final event emitted successfully"),
            Err(e) => log::error!("❌ Failed to emit final event: {}", e),
        }
    } else {
        log::info!("⚠️ No app_handle for final preview event");
    }

    let elapsed = start_time.elapsed();
    log::debug!(
        "✅ [Candle] Buffer transcription completed in {:.2}s",
        elapsed.as_secs_f32()
    );

    Ok(transcript)
}

/// Get the path to the Whisper model (synchronous version for static initialization)
///
/// Priority:
/// 1. The model configured in AppSettings (user selection or auto-detected)
/// 2. Any cached Candle model in platform-aware fallback order:
///    - macOS (CPU): lightest first to avoid OOM
///    - Other (CUDA): heaviest first for best quality
fn get_model_path_sync() -> Result<PathBuf> {
    use crate::models::utils::get_model_snapshot_dir;
    use crate::models::{ModelBackend, get_model_info};
    use crate::settings::AppSettings;

    log::debug!("🔍 Looking for Candle model...");

    let cache_dir = get_hf_cache_dir()?;

    // 1. Try the user's configured model first
    if let Ok(settings) = AppSettings::load() {
        if let Some(entry) = get_model_info(&settings.model) {
            if entry.backend == ModelBackend::Candle {
                if let Some(dir) = get_model_snapshot_dir(&cache_dir, entry.repo_id) {
                    if dir.join("model.safetensors").exists() {
                        log::debug!("✅ Using configured model {}: {}", entry.id, dir.display());
                        return Ok(dir);
                    }
                }
            }
        }
    }

    // 2. Fallback: scan for any cached Candle model in platform-aware order
    #[cfg(target_os = "macos")]
    let fallback_ids: &[&str] = &[
        "distil-large-v3",
        "whisper-medium",
        "whisper-large-v3-turbo",
    ];
    #[cfg(not(target_os = "macos"))]
    let fallback_ids: &[&str] = &[
        "distil-large-v3",
        "whisper-large-v3-turbo",
        "whisper-medium",
    ];

    for model_id in fallback_ids {
        if let Some(entry) = get_model_info(model_id) {
            if let Some(dir) = get_model_snapshot_dir(&cache_dir, entry.repo_id) {
                if dir.join("model.safetensors").exists() {
                    log::debug!("✅ Using fallback model {}: {}", entry.id, dir.display());
                    return Ok(dir);
                }
            }
        }
    }

    Err(anyhow!(
        "No Candle-compatible Whisper model found. Please download a model via Settings."
    ))
}

/// Get HuggingFace cache directory (cross-platform)
fn get_hf_cache_dir() -> Result<PathBuf> {
    // Check environment variables in priority order
    if let Ok(cache_dir) = std::env::var("HUGGINGFACE_HUB_CACHE") {
        return Ok(PathBuf::from(cache_dir));
    }
    if let Ok(cache_dir) = std::env::var("TRANSFORMERS_CACHE") {
        return Ok(PathBuf::from(cache_dir));
    }
    if let Ok(hf_home) = std::env::var("HF_HOME") {
        return Ok(PathBuf::from(hf_home).join("hub"));
    }

    // Platform-specific default cache locations
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let cache_path = home_dir.join(".cache").join("huggingface").join("hub");

    Ok(cache_path)
}

/// Load Whisper model with CUDA support
fn load_whisper_model(model_path: &Path, device: &Device) -> Result<(Model, Tokenizer, Config)> {
    log::debug!("🔧 [Candle] Loading Whisper model with CUDA...");

    let config_file = model_path.join("config.json");
    let tokenizer_file = model_path.join("tokenizer.json");

    // Load config
    let config_content = std::fs::read_to_string(&config_file)
        .with_context(|| format!("Failed to read config file: {}", config_file.display()))?;
    let config: Config = serde_json::from_str(&config_content)
        .with_context(|| format!("Failed to parse config file: {}", config_file.display()))?;

    // Load tokenizer
    let tokenizer = Tokenizer::from_file(&tokenizer_file).map_err(|e| {
        anyhow!(
            "Failed to load tokenizer from {}: {}",
            tokenizer_file.display(),
            e
        )
    })?;

    // Search for GGUF file (any .gguf file, e.g., model.gguf, model-q4k.gguf, etc.)
    let gguf_file = std::fs::read_dir(model_path)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|s| s.to_str()) == Some("gguf"));

    // Check for safetensors file
    let safetensors_file = model_path.join("model.safetensors");

    // Load model based on available format
    let model = if let Some(gguf_path) = gguf_file {
        log::debug!(
            "🔢 [Candle] Loading quantized GGUF model: {}",
            gguf_path.display()
        );
        let vb =
            candle_transformers::quantized_var_builder::VarBuilder::from_gguf(&gguf_path, device)?;
        Model::Quantized(m::quantized_model::Whisper::load(&vb, config.clone())?)
    } else if safetensors_file.exists() {
        log::debug!(
            "📦 [Candle] Loading safetensors model: {}",
            safetensors_file.display()
        );
        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[safetensors_file], m::DTYPE, device)? };
        Model::Normal(m::model::Whisper::load(&vb, config.clone())?)
    } else {
        return Err(anyhow!(
            "No model file found in {}. Expected .gguf or model.safetensors file",
            model_path.display()
        ));
    };

    log::debug!("✅ [Candle] Model loaded successfully");
    Ok((model, tokenizer, config))
}

/// Convert audio samples to mel spectrogram
fn audio_to_mel(samples: &[f32], config: &Config, device: &Device) -> Result<Tensor> {
    log::debug!("🎵 [Candle] Converting audio to mel spectrogram...");

    // Load mel filters based on model configuration
    let mel_bytes = match config.num_mel_bins {
        80 => include_bytes!("../../../mel_filters.bytes").as_slice(),
        128 => include_bytes!("../../../mel_filters_128.bytes").as_slice(),
        nmel => {
            return Err(anyhow!(
                "Unsupported num_mel_bins: {} (only 80 and 128 supported)",
                nmel
            ));
        }
    };

    // Convert mel filters from bytes to f32 array (64320 bytes = 16080 f32 values)
    let mel_bytes_len = (mel_bytes.len() / 4) * 4; // Ensure 4-byte alignment
    let mel_bytes = &mel_bytes[..mel_bytes_len];
    let mut mel_filters = vec![0f32; mel_bytes_len / 4];
    <byteorder::LittleEndian as byteorder::ByteOrder>::read_f32_into(mel_bytes, &mut mel_filters);

    // Convert to mel spectrogram using Candle's audio processing
    let mel = audio::pcm_to_mel(config, samples, &mel_filters);
    let mel_len = mel.len();

    // Convert to tensor with proper shape
    let mel_tensor = Tensor::from_vec(
        mel,
        (1, config.num_mel_bins, mel_len / config.num_mel_bins),
        device,
    )?;

    log::debug!(
        "✅ [Candle] Mel spectrogram created: shape {:?}",
        mel_tensor.dims()
    );
    Ok(mel_tensor)
}

// WAV reading and resampling live in the shared echo-audio crate
// (echo_audio::wav::read_wav_mono_16k) - no local duplicates.

// ==================== TRANSCRIPTION BACKEND INTERFACE ====================

use super::TranscriptionBackend;

/// Candle transcription backend implementation
///
/// Pure Rust implementation using HuggingFace Candle framework.
/// Supports CUDA acceleration with GGUF quantized models and safetensors.
///
/// **Advantages:**
/// - Pure Rust - no C++ FFI issues
/// - Active development by HuggingFace
/// - Supports INT8/q4k/q8k quantization
/// - Better error messages
pub struct CandleWhisperBackend;

impl TranscriptionBackend for CandleWhisperBackend {
    async fn transcribe_samples(&self, samples: &[f32]) -> Result<String> {
        transcribe_audio_buffer(samples).await
    }

    fn backend_name(&self) -> &'static str {
        "Candle (Pure Rust)"
    }
}

/// Model management functions
pub mod model {
    use super::*;

    /// Download a model from Hugging Face
    #[allow(dead_code)]
    pub async fn download_model(_model_id: &str) -> Result<PathBuf> {
        // TODO: Use existing downloader to fetch from HF
        // TODO: Cache in app data directory
        // TODO: Return the downloaded path

        Err(anyhow!("Model download not implemented yet"))
    }

    /// Get the default model directory
    #[allow(dead_code)]
    pub fn get_model_dir() -> Result<PathBuf> {
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Home directory not found"))?;
        let model_dir = dirs::data_dir()
            .unwrap_or(home_dir.join("AppData").join("Roaming"))
            .join("echophrase")
            .join("models");

        Ok(model_dir)
    }
}
