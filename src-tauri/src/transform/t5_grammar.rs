// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! T5 Grammar Correction using ONNX Runtime
//!
//! Drives any T5-family encoder/decoder grammar model registered in
//! `crate::models::MODEL_REGISTRY` with `category = Transformation`. The
//! active model is read from `settings.transform_model`; switching it
//! invalidates the cache and the next `correct()` reloads from disk.

use anyhow::{Context, Result, anyhow};
use ndarray::{Array2, Array3};
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Value,
};
use parking_lot::Mutex;
use std::path::Path;
use std::sync::LazyLock;
use tokenizers::Tokenizer;

use crate::models::{ModelRegistryEntry, get_model_info};
// Default model id when settings have no `transform_model` value.
use crate::settings::DEFAULT_TRANSFORM_MODEL_ID;

// ============================================================================
// Constants
// ============================================================================

/// Maximum output length for grammar correction
const MAX_OUTPUT_LENGTH: usize = 512;

/// T5 special tokens
const PAD_TOKEN_ID: i64 = 0;
const EOS_TOKEN_ID: i64 = 1;
const DECODER_START_TOKEN_ID: i64 = 0; // T5 uses pad as decoder start

// ============================================================================
// Cached Model State
// ============================================================================

struct CachedT5Grammar {
    model_id: String,
    encoder: Session,
    init_decoder: Session,
    tokenizer: Tokenizer,
    /// Pulled from `ModelRegistryEntry::prompt_prefix` at load time.
    prompt_prefix: Option<&'static str>,
}

static CACHED_T5: LazyLock<Mutex<Option<CachedT5Grammar>>> = LazyLock::new(|| Mutex::new(None));

// ============================================================================
// Public API
// ============================================================================

/// T5 Grammar Corrector
pub struct T5GrammarCorrector;

impl T5GrammarCorrector {
    /// Apply grammar correction to input text using whichever transformation
    /// model is currently selected in settings (`settings.transform_model`).
    pub fn correct(text: &str) -> Result<String> {
        if !crate::is_onnx_runtime_available() {
            return Err(anyhow!(
                "ONNX Runtime is not available - grammar correction disabled"
            ));
        }

        // The T5 grammar tiers are English-only: majority-Han text (e.g.
        // Chinese output from Qwen3-ASR) must bypass the transform untouched.
        if crate::transform::opencc::is_majority_han(text) {
            log::info!("Transform skipped: majority-Han text (grammar model is English-only)");
            return Ok(text.to_string());
        }

        let active_id = active_transform_model_id();
        let entry = registry_entry_for(&active_id)?;

        let mut cache = CACHED_T5.lock();

        // Reload if the active model id changed (or first use)
        let needs_reload = match cache.as_ref() {
            Some(existing) => existing.model_id != entry.id,
            None => true,
        };
        if needs_reload {
            *cache = Some(load_t5_model(entry)?);
        }

        let model = cache.as_mut().unwrap();
        let prefixed: String = match model.prompt_prefix {
            Some(prefix) => format!("{prefix}{text}"),
            None => text.to_string(),
        };
        run_t5_inference(model, &prefixed)
    }

    /// Check if the T5 model is loaded
    pub fn is_loaded() -> bool {
        CACHED_T5.lock().is_some()
    }

    /// Preload the T5 model (call during app startup if transform is enabled)
    pub fn preload() -> Result<()> {
        if !crate::is_onnx_runtime_available() {
            return Err(anyhow!(
                "ONNX Runtime is not available - cannot preload grammar model"
            ));
        }

        let active_id = active_transform_model_id();
        let entry = registry_entry_for(&active_id)?;

        let mut cache = CACHED_T5.lock();
        let needs_reload = match cache.as_ref() {
            Some(existing) => existing.model_id != entry.id,
            None => true,
        };
        if needs_reload {
            *cache = Some(load_t5_model(entry)?);
        }
        Ok(())
    }

    /// Unload the model to free memory
    pub fn unload() {
        *CACHED_T5.lock() = None;
    }
}

// ============================================================================
// Settings + registry plumbing
// ============================================================================

fn active_transform_model_id() -> String {
    crate::settings::AppSettings::load()
        .ok()
        .and_then(|s| s.transform_model)
        .unwrap_or_else(|| DEFAULT_TRANSFORM_MODEL_ID.to_string())
}

fn registry_entry_for(model_id: &str) -> Result<&'static ModelRegistryEntry> {
    get_model_info(model_id)
        .ok_or_else(|| anyhow!("Transform model '{}' is not in the registry", model_id))
}

// ============================================================================
// Model Loading
// ============================================================================

fn load_t5_model(entry: &'static ModelRegistryEntry) -> Result<CachedT5Grammar> {
    let model_dir = entry.resolve_model_dir().ok_or_else(|| {
        anyhow!(
            "Transform model '{}' is not present on disk. Bundled={}, repo='{}'",
            entry.id,
            entry.bundled,
            entry.repo_id
        )
    })?;

    let (encoder_rel, decoder_rel, tokenizer_rel) = pick_t5_files(entry).with_context(|| {
        format!(
            "Transform model '{}' missing required encoder/decoder/tokenizer files in registry entry",
            entry.id
        )
    })?;

    let encoder_path = model_dir.join(encoder_rel);
    let decoder_path = model_dir.join(decoder_rel);
    let tokenizer_path = model_dir.join(tokenizer_rel);

    log::info!(
        "Loading transform model '{}' from {:?} (encoder={}, decoder={}, tokenizer={})",
        entry.id,
        model_dir,
        encoder_rel,
        decoder_rel,
        tokenizer_rel,
    );

    let session_builder = || {
        Session::builder()
            .map_err(|e| anyhow!("Failed to create session builder: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow!("Failed to set optimization level: {}", e))
    };

    let encoder = session_builder()?
        .commit_from_file(&encoder_path)
        .map_err(|e| anyhow!("Failed to load encoder from {:?}: {}", encoder_path, e))?;

    let init_decoder = session_builder()?
        .commit_from_file(&decoder_path)
        .map_err(|e| anyhow!("Failed to load decoder from {:?}: {}", decoder_path, e))?;

    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| anyhow!("Failed to load tokenizer from {:?}: {}", tokenizer_path, e))?;

    log::info!("Transform model '{}' loaded", entry.id);

    Ok(CachedT5Grammar {
        model_id: entry.id.to_string(),
        encoder,
        init_decoder,
        tokenizer,
        prompt_prefix: entry.prompt_prefix,
    })
}

/// Pick (encoder_file, decoder_file, tokenizer_file) from a registry entry's
/// `model_files` list by simple convention: first .onnx is encoder, second
/// .onnx is decoder, the entry containing `tokenizer.json` (but not
/// `tokenizer_config.json`) is the tokenizer.
fn pick_t5_files(entry: &ModelRegistryEntry) -> Result<(&'static str, &'static str, &'static str)> {
    let onnx_files: Vec<&'static str> = entry
        .model_files
        .iter()
        .copied()
        .filter(|f| f.ends_with(".onnx"))
        .collect();
    let encoder = onnx_files
        .first()
        .copied()
        .ok_or_else(|| anyhow!("no encoder .onnx file in model_files"))?;
    let decoder = onnx_files
        .get(1)
        .copied()
        .ok_or_else(|| anyhow!("no decoder .onnx file in model_files"))?;
    let tokenizer = entry
        .model_files
        .iter()
        .copied()
        .find(|f| {
            let name = Path::new(f)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            name == "tokenizer.json"
        })
        .ok_or_else(|| anyhow!("no tokenizer.json in model_files"))?;
    Ok((encoder, decoder, tokenizer))
}

// ============================================================================
// Inference
// ============================================================================

fn run_t5_inference(model: &mut CachedT5Grammar, text: &str) -> Result<String> {
    // Tokenize input
    let encoding = model
        .tokenizer
        .encode(text, true)
        .map_err(|e| anyhow!("Tokenization failed: {}", e))?;

    let input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
    let attention_mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&m| m as i64)
        .collect();

    let seq_len = input_ids.len();

    // Create input tensors [batch=1, seq_len]
    let input_ids_array = Array2::from_shape_vec((1, seq_len), input_ids)?;
    let attention_mask_array = Array2::from_shape_vec((1, seq_len), attention_mask)?;

    // Run encoder
    let encoder_outputs = run_encoder(model, &input_ids_array, &attention_mask_array)?;

    // Generate output tokens
    let output_ids = generate_tokens(model, &encoder_outputs, &attention_mask_array)?;

    // Decode output tokens
    let output_text = model
        .tokenizer
        .decode(&output_ids, true)
        .map_err(|e| anyhow!("Decoding failed: {}", e))?;

    Ok(output_text.trim().to_string())
}

fn run_encoder(
    model: &mut CachedT5Grammar,
    input_ids: &Array2<i64>,
    attention_mask: &Array2<i64>,
) -> Result<Array3<f32>> {
    let input_ids_value =
        Value::from_array(input_ids.clone()).map_err(|e| anyhow!("Input tensor error: {}", e))?;
    let attention_mask_value = Value::from_array(attention_mask.clone())
        .map_err(|e| anyhow!("Attention mask tensor error: {}", e))?;

    let outputs = model
        .encoder
        .run(ort::inputs![input_ids_value, attention_mask_value])
        .map_err(|e| anyhow!("Encoder inference failed: {}", e))?;

    // Extract encoder hidden states
    let (shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| anyhow!("Failed to extract encoder output: {}", e))?;

    // Convert to Array3 [batch, seq, hidden]
    let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    if dims.len() != 3 {
        return Err(anyhow!("Expected 3D encoder output, got {}D", dims.len()));
    }

    let encoder_output = Array3::from_shape_vec((dims[0], dims[1], dims[2]), data.to_vec())
        .map_err(|e| anyhow!("Failed to reshape encoder output: {}", e))?;

    Ok(encoder_output)
}

fn generate_tokens(
    model: &mut CachedT5Grammar,
    encoder_outputs: &Array3<f32>,
    encoder_attention_mask: &Array2<i64>,
) -> Result<Vec<u32>> {
    let mut output_ids = vec![DECODER_START_TOKEN_ID];

    for _ in 0..MAX_OUTPUT_LENGTH {
        // Use init_decoder for all tokens (simpler, no KV caching needed)
        // This is slower but works without complex state management
        let next_token =
            run_init_decoder(model, encoder_outputs, encoder_attention_mask, &output_ids)?;

        output_ids.push(next_token);

        // Stop if EOS token
        if next_token == EOS_TOKEN_ID {
            break;
        }
    }

    // Convert to u32 for tokenizer (skip start token)
    Ok(output_ids[1..]
        .iter()
        .filter(|&&id| id != EOS_TOKEN_ID && id != PAD_TOKEN_ID)
        .map(|&id| id as u32)
        .collect())
}

fn run_init_decoder(
    model: &mut CachedT5Grammar,
    encoder_outputs: &Array3<f32>,
    encoder_attention_mask: &Array2<i64>,
    decoder_input_ids: &[i64],
) -> Result<i64> {
    let decoder_ids_array =
        Array2::from_shape_vec((1, decoder_input_ids.len()), decoder_input_ids.to_vec())?;

    let encoder_value = Value::from_array(encoder_outputs.clone())
        .map_err(|e| anyhow!("Encoder output tensor error: {}", e))?;
    let encoder_mask_value = Value::from_array(encoder_attention_mask.clone())
        .map_err(|e| anyhow!("Encoder mask tensor error: {}", e))?;
    let decoder_ids_value = Value::from_array(decoder_ids_array)
        .map_err(|e| anyhow!("Decoder input tensor error: {}", e))?;

    // Input order: input_ids, encoder_attention_mask, encoder_hidden_states
    let outputs = model
        .init_decoder
        .run(ort::inputs![
            decoder_ids_value,
            encoder_mask_value,
            encoder_value
        ])
        .map_err(|e| anyhow!("Init decoder inference failed: {}", e))?;

    // Get logits and find argmax for next token
    let (shape, data) = outputs[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| anyhow!("Failed to extract logits: {}", e))?;

    // logits shape: [batch, seq, vocab] - get last token's logits
    let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    if dims.len() != 3 {
        return Err(anyhow!("Expected 3D logits, got {}D", dims.len()));
    }

    let vocab_size = dims[2];
    let seq_len = dims[1];
    // Get logits for last position
    let last_pos_start = (seq_len - 1) * vocab_size;
    let last_token_logits = &data[last_pos_start..last_pos_start + vocab_size];

    // Argmax over vocab dimension
    let next_token = last_token_logits
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, _)| idx as i64)
        .unwrap_or(EOS_TOKEN_ID);

    Ok(next_token)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grammar_correction() {
        // Test: "Tset one 2 three" -> should fix "Tset" to "Test"
        // May also normalize numbers and add punctuation
        let input = "Tset one 2 three";
        let result = T5GrammarCorrector::correct(input);

        match result {
            Ok(output) => {
                log::debug!("Input:  '{}'", input);
                log::debug!("Output: '{}'", output);

                // Check that "Tset" was corrected to "Test"
                assert!(
                    output.to_lowercase().contains("test"),
                    "Expected 'Tset' to be corrected to 'Test', got: {}",
                    output
                );

                // The model may or may not normalize numbers - just check it ran
                assert!(!output.is_empty(), "Output should not be empty");
            }
            Err(e) => {
                // If model files aren't available, skip the test
                if e.to_string().contains("not found")
                    || e.to_string().contains("not present on disk")
                {
                    log::debug!("Skipping test: T5 model files not found");
                    return;
                }
                panic!("Grammar correction failed: {}", e);
            }
        }
    }

    #[test]
    fn test_pick_t5_files_optimum_root_layout() {
        let entry = get_model_info("t5-mini-grammar").expect("t5-mini-grammar in registry");
        let (enc, dec, _tok) = pick_t5_files(entry).expect("pick succeeds");
        assert_eq!(enc, "encoder_model_quantized.onnx");
        assert_eq!(dec, "init_decoder_model_quantized.onnx");
    }
}
