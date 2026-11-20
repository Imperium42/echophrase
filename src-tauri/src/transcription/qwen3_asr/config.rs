// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Qwen3-ASR model configuration loaded from the repo's `config.json`.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Top-level model configuration (`config.json` in the model directory).
// Fields deserialized from config.json; not all are used at runtime.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Qwen3AsrConfig {
    pub encoder: EncoderConfig,
    pub decoder: DecoderConfig,
    pub mel: MelParams,
    pub special_tokens: SpecialTokens,
    /// Storage dtype of `embed_tokens.bin` (defaults to float32 when absent).
    #[serde(default)]
    pub embed_tokens_dtype: EmbedDtype,
}

/// Storage dtype for `embed_tokens.bin`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbedDtype {
    #[default]
    Float32,
    Float16,
}

impl EmbedDtype {
    /// Bytes per element for this dtype.
    pub fn bytes_per_element(self) -> usize {
        match self {
            Self::Float32 => 4,
            Self::Float16 => 2,
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EncoderConfig {
    pub num_mel_bins: usize,
    pub output_dim: usize,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DecoderConfig {
    pub num_layers: usize,
    pub hidden_size: usize,
    pub num_attention_heads: usize,
    pub num_key_value_heads: usize,
    pub head_dim: usize,
    pub vocab_size: usize,
}

/// Mel parameters from config.json, validated against the compiled-in
/// constants in `mel.rs` at load time.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct MelParams {
    pub sample_rate: u32,
    pub n_fft: usize,
    pub hop_length: usize,
    pub n_mels: usize,
    pub fmin: f64,
    pub fmax: f64,
}

/// Special token IDs used by the prompt template and decode loop.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SpecialTokens {
    pub eos_token_ids: Vec<i64>,
    pub pad_token_id: i64,
    pub im_start_token_id: i64,
    pub im_end_token_id: i64,
    pub audio_start_token_id: i64,
    pub audio_end_token_id: i64,
    pub audio_pad_token_id: i64,
    /// Separates the language prefix from the transcription text.
    #[serde(default = "default_asr_text_token_id")]
    pub asr_text_token_id: i64,
}

fn default_asr_text_token_id() -> i64 {
    151704
}

impl Qwen3AsrConfig {
    /// Load and validate `config.json` from the model directory.
    pub fn load(model_dir: &Path) -> Result<Self> {
        let config_path = model_dir.join("config.json");
        let data = fs::read_to_string(&config_path)
            .with_context(|| format!("read {}", config_path.display()))?;
        let config: Self = serde_json::from_str(&data)
            .with_context(|| format!("parse {}", config_path.display()))?;

        let st = &config.special_tokens;
        let valid = st.pad_token_id >= 0
            && st.im_start_token_id >= 0
            && st.im_end_token_id >= 0
            && st.audio_start_token_id >= 0
            && st.audio_end_token_id >= 0
            && st.audio_pad_token_id >= 0
            && st.asr_text_token_id >= 0
            && !st.eos_token_ids.is_empty()
            && st.eos_token_ids.iter().all(|&id| id >= 0);
        anyhow::ensure!(
            valid,
            "config.json: special_tokens contains negative or missing IDs"
        );
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"{
        "encoder": { "num_mel_bins": 128, "output_dim": 1024 },
        "decoder": { "num_layers": 28, "hidden_size": 1024, "num_attention_heads": 16,
                     "num_key_value_heads": 8, "head_dim": 128, "vocab_size": 151936 },
        "mel": { "sample_rate": 16000, "n_fft": 400, "hop_length": 160,
                 "n_mels": 128, "fmin": 0, "fmax": 8000 },
        "special_tokens": { "eos_token_ids": [151643, 151645], "pad_token_id": 151643,
                            "im_start_token_id": 151644, "im_end_token_id": 151645,
                            "audio_start_token_id": 151669, "audio_end_token_id": 151670,
                            "audio_pad_token_id": 151676, "asr_text_token_id": 151704 },
        "embed_tokens_dtype": "float16"
    }"#;

    #[test]
    fn deserializes_config() {
        let config: Qwen3AsrConfig = serde_json::from_str(VALID).unwrap();
        assert_eq!(config.decoder.num_layers, 28);
        assert_eq!(config.decoder.vocab_size, 151936);
        assert_eq!(config.embed_tokens_dtype, EmbedDtype::Float16);
        assert_eq!(config.embed_tokens_dtype.bytes_per_element(), 2);
    }

    #[test]
    fn embed_dtype_defaults_to_float32() {
        let json = VALID.replace(r#""embed_tokens_dtype": "float16""#, r#""unused": 0"#);
        let config: Qwen3AsrConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.embed_tokens_dtype, EmbedDtype::Float32);
    }
}
