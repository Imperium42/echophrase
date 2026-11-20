// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Core ONNX inference for Qwen3-ASR: encoder, decoder prefill, and the
//! KV-cache autoregressive decode loop.

use anyhow::{Context, Result, anyhow};
use ndarray::{Array1, Array2, Array3};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Value;
use std::collections::HashMap;
use std::path::Path;
use tokenizers::Tokenizer;

use super::config::{EmbedDtype, Qwen3AsrConfig};
use super::mel::{self, log_mel_spectrogram};
use super::prompt::{build_prompt_ids, get_audio_pad_start, get_feat_extract_output_lengths};

/// Loaded Qwen3-ASR model: three ORT sessions plus host-side lookup tables.
pub struct Qwen3AsrModel {
    encoder: Session,
    decoder_init: Session,
    decoder_step: Session,
    /// FP32 embedding matrix `[vocab, hidden]` cached for fast per-step lookup.
    /// `decoder_init` has the embedding table in its graph (for the prefill
    /// scatter); `decoder_step` takes pre-looked-up `input_embeds` to keep the
    /// table out of the hot decode session.
    embed_tokens: Array2<f32>,
    config: Qwen3AsrConfig,
    tokenizer: Tokenizer,
    /// Cache of language name -> prompt token IDs for ` {Name}`.
    language_cache: HashMap<String, Vec<i64>>,
}

impl Qwen3AsrModel {
    /// Load all model components from the model directory (int4 export layout).
    pub fn load(model_dir: &Path) -> Result<Self> {
        let config = Qwen3AsrConfig::load(model_dir)?;

        anyhow::ensure!(
            config.mel.n_mels == mel::N_MELS
                && config.mel.n_fft == mel::N_FFT
                && config.mel.hop_length == mel::HOP_LENGTH,
            "Qwen3-ASR config.json mel params do not match compiled-in constants"
        );

        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer from {tokenizer_path:?}: {e}"))?;

        let encoder = create_session(&model_dir.join("encoder.int4.onnx"))?;
        let decoder_init = create_session(&model_dir.join("decoder_init.int4.onnx"))?;
        let decoder_step = create_session(&model_dir.join("decoder_step.int4.onnx"))?;

        let embed_path = model_dir.join("embed_tokens.bin");
        let embed_tokens = load_embed_cache(&embed_path, &config)?;
        log::info!(
            "Qwen3-ASR embedding cache loaded: {:?}",
            embed_tokens.shape()
        );

        Ok(Self {
            encoder,
            decoder_init,
            decoder_step,
            embed_tokens,
            config,
            tokenizer,
            language_cache: HashMap::new(),
        })
    }

    /// Full pipeline for one audio chunk: samples -> raw decoded text
    /// (including the `language <Name>` prefix, stripped by the caller).
    pub fn transcribe(
        &mut self,
        samples: &[f32],
        max_tokens: usize,
        language: Option<&str>,
    ) -> Result<String> {
        let mel = log_mel_spectrogram(samples);
        let mel_frames = mel.shape()[2];
        let audio_tokens = get_feat_extract_output_lengths(mel_frames);
        if audio_tokens == 0 {
            return Ok(String::new());
        }

        let audio_features = self.encode(&mel)?;
        let encoder_frames = audio_features.shape()[1];
        anyhow::ensure!(
            encoder_frames == audio_tokens,
            "Qwen3-ASR encoder produced {encoder_frames} frames, expected {audio_tokens} \
             (mel_frames={mel_frames})"
        );

        let lang_tokens = language.map(|lang| self.language_token_ids(lang));
        self.greedy_decode(&audio_features, max_tokens, lang_tokens.as_deref())
    }

    /// Run the encoder: mel `[1, 128, T]` -> audio features `[1, T', 1024]`.
    fn encode(&mut self, mel: &Array3<f32>) -> Result<Array3<f32>> {
        let mel_value = Value::from_array(mel.clone()).context("mel tensor")?;
        let outputs = self
            .encoder
            .run(ort::inputs!["mel" => mel_value])
            .context("Qwen3-ASR encoder inference failed")?;

        let (shape, data) = outputs
            .get("audio_features")
            .ok_or_else(|| anyhow!("Missing 'audio_features' output"))?
            .try_extract_tensor::<f32>()
            .context("extract audio_features")?;
        let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        anyhow::ensure!(dims.len() == 3, "expected 3D audio_features, got {dims:?}");
        Ok(Array3::from_shape_vec(
            (dims[0], dims[1], dims[2]),
            data.to_vec(),
        )?)
    }

    /// Greedy KV-cache decode: prefill via `decoder_init`, then single-token
    /// steps via `decoder_step`, feeding each step's `present_*` KV outputs
    /// back as the next step's `past_*` inputs, until EOS or `max_tokens`.
    fn greedy_decode(
        &mut self,
        audio_features: &Array3<f32>,
        max_tokens: usize,
        language_token_ids: Option<&[i64]>,
    ) -> Result<String> {
        let max_tokens = max_tokens.min(4096);
        let audio_token_count = audio_features.shape()[1];
        let prompt_ids = build_prompt_ids(
            &self.config.special_tokens,
            audio_token_count,
            language_token_ids,
        );
        let seq_len = prompt_ids.len();

        let audio_start =
            get_audio_pad_start(&prompt_ids, self.config.special_tokens.audio_pad_token_id)
                .ok_or_else(|| anyhow!("No audio_pad tokens in prompt"))?;

        let input_ids = Array2::<i64>::from_shape_vec((1, seq_len), prompt_ids)?;
        let position_ids = Array2::<i64>::from_shape_fn((1, seq_len), |(_, j)| j as i64);
        let audio_offset = Array1::<i64>::from_elem(1, audio_start as i64);

        // Prefill: decoder_init handles embedding lookup + audio scatter internally.
        let mut init_outputs = self
            .decoder_init
            .run(ort::inputs![
                "input_ids"      => Value::from_array(input_ids)?,
                "position_ids"   => Value::from_array(position_ids)?,
                "audio_features" => Value::from_array(audio_features.clone())?,
                "audio_offset"   => Value::from_array(audio_offset)?,
            ])
            .context("Qwen3-ASR decoder_init inference failed")?;

        let mut current_token = {
            let (shape, data) = init_outputs
                .get("logits")
                .ok_or_else(|| anyhow!("Missing 'logits' from decoder_init"))?
                .try_extract_tensor::<f32>()
                .context("extract prefill logits")?;
            argmax_last_position(shape, data)?
        };
        let mut keys = init_outputs
            .remove("present_keys")
            .ok_or_else(|| anyhow!("Missing 'present_keys' from decoder_init"))?;
        let mut values = init_outputs
            .remove("present_values")
            .ok_or_else(|| anyhow!("Missing 'present_values' from decoder_init"))?;

        let eos = &self.config.special_tokens.eos_token_ids;
        let mut output_tokens = vec![current_token];
        let hidden_size = self.config.decoder.hidden_size;
        let mut pos = seq_len as i64;

        while !eos.contains(&current_token) && output_tokens.len() < max_tokens {
            // Host-side embedding lookup for the new token.
            anyhow::ensure!(
                current_token >= 0 && (current_token as usize) < self.embed_tokens.nrows(),
                "decoder produced out-of-range token ID {current_token}"
            );
            let row = self.embed_tokens.row(current_token as usize);
            let mut token_embed = Array3::<f32>::zeros((1, 1, hidden_size));
            token_embed.slice_mut(ndarray::s![0, 0, ..]).assign(&row);
            let step_pos = Array2::<i64>::from_elem((1, 1), pos);

            // KV cache is moved in by value; ORT references the memory directly.
            let mut step_outputs = self
                .decoder_step
                .run(ort::inputs![
                    "input_embeds" => Value::from_array(token_embed)?,
                    "position_ids" => Value::from_array(step_pos)?,
                    "past_keys"    => keys,
                    "past_values"  => values,
                ])
                .context("Qwen3-ASR decoder_step inference failed")?;

            current_token = {
                let (shape, data) = step_outputs
                    .get("logits")
                    .ok_or_else(|| anyhow!("Missing 'logits' from decoder_step"))?
                    .try_extract_tensor::<f32>()
                    .context("extract step logits")?;
                argmax_last_position(shape, data)?
            };
            output_tokens.push(current_token);
            pos += 1;

            keys = step_outputs
                .remove("present_keys")
                .ok_or_else(|| anyhow!("Missing 'present_keys' from decoder_step"))?;
            values = step_outputs
                .remove("present_values")
                .ok_or_else(|| anyhow!("Missing 'present_values' from decoder_step"))?;
        }

        let eos_reached = eos.contains(&current_token);
        if !eos_reached {
            log::warn!("Qwen3-ASR: max_tokens ({max_tokens}) reached without EOS");
        }

        // With no forced language prefix, a completed decode missing the
        // <asr_text> separator means degenerate output (e.g. int4 noise on
        // non-speech audio) - return empty rather than garbage.
        let asr_text_id = self.config.special_tokens.asr_text_token_id;
        if language_token_ids.is_none() && eos_reached && !output_tokens.contains(&asr_text_id) {
            log::warn!(
                "Qwen3-ASR: no <asr_text> token in {} output tokens; returning empty transcription",
                output_tokens.len()
            );
            return Ok(String::new());
        }

        let ids: Vec<u32> = output_tokens
            .iter()
            .filter(|&&id| id >= 0)
            .map(|&id| id as u32)
            .collect();
        let decoded = self
            .tokenizer
            .decode(&ids, true)
            .map_err(|e| anyhow!("Qwen3-ASR decode failed: {e}"))?;
        // Belt and suspenders: special-token skipping (`skip_special_tokens: true`
        // above) is evidently unreliable for the `<asr_text>` marker (token id
        // 151704) in auto-language free generation, so strip any literal
        // occurrence that leaks through into the decoded string.
        Ok(strip_asr_text_tag(&decoded))
    }

    /// Tokenize ` {Name}` for the forced language prefix, cached per language.
    fn language_token_ids(&mut self, language: &str) -> Vec<i64> {
        if let Some(cached) = self.language_cache.get(language) {
            return cached.clone();
        }
        let spaced = format!(" {language}");
        let tokens: Vec<i64> = self
            .tokenizer
            .encode(spaced.as_str(), false)
            .map(|enc| enc.get_ids().iter().map(|&id| id as i64).collect())
            .unwrap_or_default();
        log::info!("Qwen3-ASR: language hint {language:?} -> {tokens:?}");
        self.language_cache
            .insert(language.to_string(), tokens.clone());
        tokens
    }
}

/// Create an ORT session honoring the user's execution-provider preference,
/// mirroring the policy in `onnx_backend.rs` (auto-device by default).
fn create_session(path: &Path) -> Result<Session> {
    use crate::transcription::execution_provider::{
        ExecutionProviderType, build_execution_providers, parse_execution_provider,
    };
    use ort::session::builder::AutoDevicePolicy;

    anyhow::ensure!(path.exists(), "Qwen3-ASR model file missing: {path:?}");

    let ep_preference = crate::settings::AppSettings::load()
        .ok()
        .and_then(|s| s.execution_provider)
        .and_then(|s| parse_execution_provider(&s));

    let b = Session::builder().map_err(|e| anyhow!("session builder: {e}"))?;
    let b = match ep_preference {
        None => b
            .with_auto_device(AutoDevicePolicy::MaxPerformance)
            .map_err(|e| anyhow!("auto device: {e}"))?,
        Some(ExecutionProviderType::Cpu) => b,
        Some(explicit) => {
            let providers = build_execution_providers(Some(explicit));
            b.with_execution_providers(providers)
                .map_err(|e| anyhow!("execution providers: {e}"))?
        }
    };
    let mut b = b
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| anyhow!("optimization level: {e}"))?;
    b.commit_from_file(path)
        .map_err(|e| anyhow!("Failed to load Qwen3-ASR session from {path:?}: {e}"))
}

/// Load `embed_tokens.bin` (`[vocab, hidden]`, row-major) into an f32 matrix,
/// casting from FP16 when the config says so.
fn load_embed_cache(path: &Path, config: &Qwen3AsrConfig) -> Result<Array2<f32>> {
    let vocab_size = config.decoder.vocab_size;
    let hidden_size = config.decoder.hidden_size;
    let bpe = config.embed_tokens_dtype.bytes_per_element();
    let expected_bytes = vocab_size * hidden_size * bpe;

    anyhow::ensure!(path.exists(), "Qwen3-ASR embedding file missing: {path:?}");
    let file_size = path.metadata()?.len() as usize;
    anyhow::ensure!(
        file_size == expected_bytes,
        "embed_tokens.bin size {file_size} != expected {expected_bytes} \
         ({vocab_size}x{hidden_size}, dtype={:?})",
        config.embed_tokens_dtype
    );

    let data = std::fs::read(path)?;
    let float_data: Vec<f32> = match config.embed_tokens_dtype {
        EmbedDtype::Float16 => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| f16_to_f32(u16::from_le_bytes(*c)))
            .collect(),
        EmbedDtype::Float32 => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect(),
    };
    Ok(Array2::from_shape_vec(
        (vocab_size, hidden_size),
        float_data,
    )?)
}

/// Convert an IEEE 754 half-precision float (u16 bits) to f32.
#[inline]
fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let frac = (h & 0x3FF) as u32;

    if exp == 0 {
        if frac == 0 {
            f32::from_bits(sign << 31)
        } else {
            // Subnormal: normalize
            let mut e = exp;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e += 1;
            }
            f &= 0x3FF;
            let exp32 = 127 - 15 - e + 1;
            f32::from_bits((sign << 31) | (exp32 << 23) | (f << 13))
        }
    } else if exp == 31 {
        f32::from_bits((sign << 31) | (0xFF << 23) | (frac << 13))
    } else {
        let exp32 = exp + 127 - 15;
        f32::from_bits((sign << 31) | (exp32 << 23) | (frac << 13))
    }
}

/// Argmax over the vocab dimension at the LAST sequence position of a
/// `[1, T, vocab]` logits tensor given as (shape, contiguous row-major data).
fn argmax_last_position(shape: &ort::value::Shape, data: &[f32]) -> Result<i64> {
    let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    anyhow::ensure!(
        dims.len() == 3 && dims[0] == 1 && dims[1] >= 1,
        "argmax: expected logits shape [1, T>=1, vocab], got {dims:?}"
    );
    let vocab = dims[2];
    let start = (dims[1] - 1) * vocab;
    let row = &data[start..start + vocab];
    let best = row
        .iter()
        .enumerate()
        .fold((0usize, f32::NEG_INFINITY), |(bi, bv), (i, &v)| {
            if v > bv { (i, v) } else { (bi, bv) }
        })
        .0;
    Ok(best as i64)
}

/// Remove any literal `<asr_text>` tag from a decoded string. Belt-and-suspenders
/// against unreliable special-token skipping for this marker (token id 151704);
/// the primary strip lives in `super::strip_language_prefix`, but this catches
/// anything that reaches the raw decode output.
fn strip_asr_text_tag(text: &str) -> String {
    const ASR_TEXT_TAG: &str = "<asr_text>";
    if text.contains(ASR_TEXT_TAG) {
        text.replace(ASR_TEXT_TAG, "")
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f16_conversion_known_values() {
        assert_eq!(f16_to_f32(0x0000), 0.0f32);
        assert_eq!(f16_to_f32(0x3C00), 1.0f32);
        assert_eq!(f16_to_f32(0xBC00), -1.0f32);
        assert_eq!(f16_to_f32(0x4000), 2.0f32);
        assert_eq!(f16_to_f32(0x3800), 0.5f32);
        assert_eq!(f16_to_f32(0x7BFF), 65504.0f32);
        assert_eq!(f16_to_f32(0x7C00), f32::INFINITY);
        assert!(f16_to_f32(0x7E00).is_nan());
        // Smallest subnormal: 2^-24
        assert!((f16_to_f32(0x0001) - 5.960_464_5e-8).abs() < 1e-12);
    }

    #[test]
    fn argmax_picks_last_position() {
        // [1, 2, 3]: position 0 favors idx 0; last position favors idx 2.
        let data = [9.0, 0.0, 0.0, 0.1, 0.2, 5.0];
        let shape = ort::value::Shape::new([1, 2, 3]);
        assert_eq!(argmax_last_position(&shape, &data).unwrap(), 2);
    }

    #[test]
    fn argmax_rejects_bad_shape() {
        let shape = ort::value::Shape::new([2, 3]);
        assert!(argmax_last_position(&shape, &[0.0; 6]).is_err());
    }
}
