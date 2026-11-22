// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};

pub mod byo_validator;
pub mod checker;
pub mod utils;

pub use checker::ModelFileChecker;
#[allow(unused_imports)]
pub use utils::{get_hf_cache_dir, get_model_snapshot_dir, repo_to_cache_name};

// ============================================================================
// Model Registry - mirrors frontend MODEL_REGISTRY in types/models.ts
// ============================================================================

/// Backend type for a model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelBackend {
    Candle,
    Onnx,
}

/// Category of a model - controls which UI surface (transcription picker vs
/// transformation picker) lists it, and which settings field tracks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelCategory {
    Transcription,
    Transformation,
}

/// Model registry entry - matches frontend ModelDefinition
pub struct ModelRegistryEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub backend: ModelBackend,
    pub category: ModelCategory,
    pub repo_id: &'static str,
    /// Subdirectory within the repo snapshot for model files (e.g., "realtime_eou_120m-v1-onnx")
    pub model_subdir: Option<&'static str>,
    /// Files to download - supports multi-file models (encoder/decoder split)
    pub model_files: &'static [&'static str],
    /// Expected total size in MB (for progress display)
    pub expected_size_mb: u64,
    /// Checker for validating model files exist (Strategy pattern)
    pub checker: &'static dyn ModelFileChecker,
    /// True if this model ships in the app bundle and lives under
    /// `<resources>/<bundled_resource_dir>` instead of the HF cache.
    pub bundled: bool,
    /// Resource subdirectory under `src-tauri/resources/` (and the platform
    /// equivalent in production bundles) for `bundled` models.
    pub bundled_resource_dir: Option<&'static str>,
    /// Optional instruction prefix prepended to inputs before tokenization
    /// (eg CoEdit expects `"Fix grammatical errors in this sentence: "`).
    pub prompt_prefix: Option<&'static str>,
    /// Model supports Eastern languages (Chinese, Japanese, Korean, etc).
    /// False = European/English only. Mirrors frontend `supportsEasternLanguages`.
    #[allow(dead_code)]
    pub supports_eastern_languages: bool,
}

impl ModelRegistryEntry {
    /// Get the primary model file (first in the list)
    #[allow(dead_code)]
    pub fn primary_file(&self) -> &'static str {
        self.model_files.first().unwrap_or(&"model.safetensors")
    }

    /// Check if this model is downloaded and complete.
    ///
    /// For bundled models, resolves the on-disk resource directory and asks
    /// the checker. For DLC models, uses the standard HF cache lookup.
    pub fn is_downloaded(&self) -> bool {
        if self.bundled {
            return match resolve_bundled_resource_dir(self) {
                Some(dir) => self.checker.is_downloaded(&dir),
                None => false,
            };
        }

        let cache_dir = match get_hf_cache_dir() {
            Ok(dir) => dir,
            Err(_) => return false,
        };

        match get_model_snapshot_dir(&cache_dir, self.repo_id) {
            Some(model_dir) => self.checker.is_downloaded(&model_dir),
            None => false,
        }
    }

    /// Resolve the on-disk directory containing this model's files.
    /// Returns the bundled resource path for `bundled` entries and the HF
    /// cache snapshot dir otherwise. None if not present.
    pub fn resolve_model_dir(&self) -> Option<std::path::PathBuf> {
        if self.bundled {
            return resolve_bundled_resource_dir(self);
        }
        let cache_dir = get_hf_cache_dir().ok()?;
        get_model_snapshot_dir(&cache_dir, self.repo_id)
    }
}

/// Resolve the bundled resource directory for a registry entry by walking
/// the four candidate locations used in production layouts (Tauri deb /
/// Windows NSIS / flat resource / Cargo dev).
fn resolve_bundled_resource_dir(entry: &ModelRegistryEntry) -> Option<std::path::PathBuf> {
    let dir_name = entry.bundled_resource_dir?;
    let exe_path = std::env::current_exe().ok()?;
    let exe_dir = exe_path.parent()?;

    let tauri_deb = exe_dir
        .join("../lib/Echophrase")
        .join("resources")
        .join(dir_name);
    if tauri_deb.exists() {
        return Some(tauri_deb);
    }
    let prod = exe_dir.join("resources").join(dir_name);
    if prod.exists() {
        return Some(prod);
    }
    let flat = exe_dir.join(dir_name);
    if flat.exists() {
        return Some(flat);
    }
    let dev = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join(dir_name);
    if dev.exists() {
        return Some(dev);
    }
    None
}

/// Get model info from model ID string
/// Returns reference to the registry entry (static lifetime)
pub fn get_model_info(model_id: &str) -> Option<&'static ModelRegistryEntry> {
    MODEL_REGISTRY.iter().find(|m| m.id == model_id)
}

/// Static model registry - matches frontend MODEL_REGISTRY
///
/// Whisper/Candle entries live in the frontend registry as commented-out
/// templates (src/lib/types/models.ts). If re-enabling, mirror the block
/// here with the corresponding checker from crate::models::checker.
pub static MODEL_REGISTRY: &[ModelRegistryEntry] = &[
    // ─────────────────────────────────────────────────────────────────────
    // Transcription - ONNX models (NVIDIA Parakeet encoder-decoder)
    // ─────────────────────────────────────────────────────────────────────
    ModelRegistryEntry {
        id: "parakeet-tdt-0.6b-v3",
        name: "NVIDIA Parakeet-TDT 0.6B",
        backend: ModelBackend::Onnx,
        category: ModelCategory::Transcription,
        repo_id: "istupakov/parakeet-tdt-0.6b-v3-onnx",
        model_subdir: None,
        model_files: &[
            "encoder-model.onnx",
            "encoder-model.onnx.data",
            "decoder_joint-model.onnx",
            "nemo128.onnx",
            "config.json",
            "vocab.txt",
        ],
        expected_size_mb: 2550,
        checker: &checker::ONNX_PARAKEET_TDT_0_6B_V3,
        bundled: false,
        bundled_resource_dir: None,
        prompt_prefix: None,
        supports_eastern_languages: false,
    },
    ModelRegistryEntry {
        id: "parakeet-eou-120m",
        name: "Parakeet Lite 120M",
        backend: ModelBackend::Onnx,
        category: ModelCategory::Transcription,
        repo_id: "altunenes/parakeet-rs",
        model_subdir: Some("realtime_eou_120m-v1-onnx"),
        model_files: &[
            "realtime_eou_120m-v1-onnx/encoder.onnx",
            "realtime_eou_120m-v1-onnx/decoder_joint.onnx",
            "realtime_eou_120m-v1-onnx/tokenizer.json",
        ],
        expected_size_mb: 481,
        checker: &checker::ONNX_PARAKEET_EOU_120M,
        bundled: false,
        bundled_resource_dir: None,
        prompt_prefix: None,
        supports_eastern_languages: false,
    },
    // ─────────────────────────────────────────────────────────────────────
    // Transcription - Qwen3-ASR (Alibaba, Apache-2.0). Community ONNX export
    // with int4 RTN decoder. 52 languages incl. Chinese dialects; first
    // supports_eastern_languages=true model. Inference lives in
    // transcription/qwen3_asr (hand-rolled ORT sessions + KV-cache decode).
    // ─────────────────────────────────────────────────────────────────────
    ModelRegistryEntry {
        id: "qwen3-asr-0.6b",
        name: "Qwen3-ASR 0.6B",
        backend: ModelBackend::Onnx,
        category: ModelCategory::Transcription,
        repo_id: "andrewleech/qwen3-asr-0.6b-onnx",
        model_subdir: None,
        model_files: &[
            "encoder.int4.onnx",
            "decoder_init.int4.onnx",
            "decoder_step.int4.onnx",
            "decoder_weights.int4.data",
            "embed_tokens.bin",
            "tokenizer.json",
            "config.json",
            "preprocessor_config.json",
        ],
        expected_size_mb: 2030,
        checker: &checker::ONNX_QWEN3_ASR_0_6B,
        bundled: false,
        bundled_resource_dir: None,
        prompt_prefix: None,
        supports_eastern_languages: true,
    },
    // ─────────────────────────────────────────────────────────────────────
    // Transformation - T5-family grammar correctors (encoder + decoder ONNX)
    //
    // Single shipping tier today: T5-Mini Grammar (visheratin, MIT, C4_200M
    // CC-BY 4.0 training data - no NC restrictions, safe for commercial
    // distribution). Reuses the existing T5GrammarCorrector inference path.
    // We use visheratin's pre-quantized `*_quantized.onnx` export and skip
    // the `decoder_*` KV-cache variants (no KV-cache in our inference loop).
    //
    // Custom Model is a coming-soon teaser - lets users bring their own
    // ONNX file (similar to the voice "custom" tier). The runtime is
    // generic; license decisions sit with the user, mirroring how LM
    // Studio / Ollama / GPT4All / llama.cpp UI wrappers handle BYO models.
    // ─────────────────────────────────────────────────────────────────────
    ModelRegistryEntry {
        id: "t5-mini-grammar",
        name: "T5-Mini Grammar",
        backend: ModelBackend::Onnx,
        category: ModelCategory::Transformation,
        repo_id: "visheratin/t5-efficient-mini-grammar-correction",
        model_subdir: None,
        model_files: &[
            "encoder_model_quantized.onnx",
            "init_decoder_model_quantized.onnx",
            "tokenizer.json",
            "tokenizer_config.json",
            "special_tokens_map.json",
            "config.json",
        ],
        expected_size_mb: 60,
        checker: &checker::ONNX_T5_MINI_GRAMMAR,
        bundled: false,
        bundled_resource_dir: None,
        prompt_prefix: None,
        supports_eastern_languages: false,
    },
];

/// Get HuggingFace cache directory name from model ID
pub fn get_hf_cache_dir_name(model_id: &str) -> Option<String> {
    get_model_info(model_id).map(|m| format!("models--{}", m.repo_id.replace('/', "--")))
}

/// Check if model uses ONNX backend
pub fn is_onnx_model(model_id: &str) -> bool {
    get_model_info(model_id)
        .map(|m| m.backend == ModelBackend::Onnx)
        .unwrap_or(false)
}

/// Convert new model ID to legacy WhisperModelInfo for backward compatibility
/// This bridges the gap during migration from enum to string model IDs
pub fn get_legacy_model_info(model_id: &str) -> Option<WhisperModelInfo> {
    // Try to get from new registry first
    if let Some(entry) = get_model_info(model_id) {
        // For legacy compat, use first file as the "primary" filename
        let primary_filename = entry.model_files.first().unwrap_or(&"model.safetensors");
        return Some(WhisperModelInfo {
            display_name: entry.name.to_string(),
            description: format!("Model: {}", entry.name),
            model_name: entry.repo_id.to_string(),
            download_url: format!("https://huggingface.co/{}", entry.repo_id),
            model_filename: primary_filename.to_string(),
            expected_size_mb: entry.expected_size_mb,
            parameters: "Unknown".to_string(),
            relative_speed: "Unknown".to_string(),
            quality: "Good".to_string(),
            min_gpu_vram_gb: 4.0,
            recommended_gpu: "RTX 3060+".to_string(),
            compute_type: match entry.backend {
                ModelBackend::Candle => "safetensors".to_string(),
                ModelBackend::Onnx => "onnx".to_string(),
            },
        });
    }
    None
}

/// Get all model files for a given model ID
#[allow(dead_code)]
pub fn get_model_files(model_id: &str) -> Option<Vec<String>> {
    get_model_info(model_id).map(|m| m.model_files.iter().map(|s| s.to_string()).collect())
}

/// Metadata for each Whisper model variant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperModelInfo {
    pub display_name: String,
    pub description: String,
    pub model_name: String,
    pub download_url: String,
    pub model_filename: String, // Actual filename in HuggingFace repo (e.g., "model.gguf", "model-q4k.gguf")
    pub expected_size_mb: u64,
    pub parameters: String,
    pub relative_speed: String,
    pub quality: String,
    pub min_gpu_vram_gb: f32,
    pub recommended_gpu: String,
    pub compute_type: String, // int8, float16, float32
}
