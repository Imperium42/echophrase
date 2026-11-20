// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use std::path::Path;

// ============================================================================
// Model File Checker - Strategy pattern for format-specific file detection
// ============================================================================

/// Trait for checking if model files exist on disk.
/// Implementations handle format-specific file detection (safetensors, ONNX, etc.)
pub trait ModelFileChecker: Send + Sync {
    /// Returns true if all required model files are present and complete
    fn is_downloaded(&self, model_dir: &Path) -> bool;

    /// Returns the primary file to check for quick validation
    #[allow(dead_code)]
    fn primary_file(&self) -> &str;

    /// Returns all required files for full validation
    fn required_files(&self) -> &[&str];
}

// ============================================================================
// Safetensors Checker - For Candle/Whisper models
// ============================================================================

/// Checker for safetensors format models (Candle backend).
/// Validates presence of model.safetensors and required config files.
#[allow(dead_code)]
pub struct SafetensorsChecker {
    pub primary: &'static str,
    pub required: &'static [&'static str],
}

impl ModelFileChecker for SafetensorsChecker {
    fn is_downloaded(&self, model_dir: &Path) -> bool {
        // Check all required files exist and are not partial downloads
        self.required_files().iter().all(|file| {
            let file_path = model_dir.join(file);
            let partial_path = model_dir.join(format!("{}.partial", file));
            file_path.exists() && !partial_path.exists()
        })
    }

    fn primary_file(&self) -> &str {
        self.primary
    }

    fn required_files(&self) -> &[&str] {
        self.required
    }
}

// ============================================================================
// ONNX Checker - For NVIDIA Parakeet and other ONNX models
// ============================================================================

/// Checker for ONNX format models (encoder-decoder architectures).
/// Validates presence of .onnx files and companion .onnx.data files.
pub struct OnnxChecker {
    #[allow(dead_code)]
    pub primary: &'static str,
    pub required: &'static [&'static str],
}

impl ModelFileChecker for OnnxChecker {
    fn is_downloaded(&self, model_dir: &Path) -> bool {
        // ONNX models may have .onnx.data companion files
        // Check all required files exist and are not partial downloads
        self.required_files().iter().all(|file| {
            let file_path = model_dir.join(file);
            let partial_path = model_dir.join(format!("{}.partial", file));
            file_path.exists() && !partial_path.exists()
        })
    }

    fn primary_file(&self) -> &str {
        self.primary
    }

    fn required_files(&self) -> &[&str] {
        self.required
    }
}

// ============================================================================
// Static Checker Instances - One per model in registry
// ============================================================================

/// Distil-Whisper Large-v3 (Safetensors) - Primary recommended model
#[allow(dead_code)]
pub static SAFETENSORS_DISTIL_LARGE_V3: SafetensorsChecker = SafetensorsChecker {
    primary: "model.safetensors",
    required: &["model.safetensors"],
};

/// Whisper Large-v3 Turbo (Safetensors)
#[allow(dead_code)]
pub static SAFETENSORS_WHISPER_LARGE_V3_TURBO: SafetensorsChecker = SafetensorsChecker {
    primary: "model.safetensors",
    required: &["model.safetensors"],
};

/// Whisper Medium (Safetensors)
#[allow(dead_code)]
pub static SAFETENSORS_WHISPER_MEDIUM: SafetensorsChecker = SafetensorsChecker {
    primary: "model.safetensors",
    required: &["model.safetensors"],
};

/// Whisper Small (Safetensors)
#[allow(dead_code)]
pub static SAFETENSORS_WHISPER_SMALL: SafetensorsChecker = SafetensorsChecker {
    primary: "model.safetensors",
    required: &["model.safetensors"],
};

/// NVIDIA Parakeet-TDT 0.6B v3 (ONNX) - Encoder-decoder architecture
pub static ONNX_PARAKEET_TDT_0_6B_V3: OnnxChecker = OnnxChecker {
    primary: "encoder-model.onnx",
    required: &[
        "encoder-model.onnx",
        "encoder-model.onnx.data",
        "decoder_joint-model.onnx",
        "nemo128.onnx",
        "config.json",
        "vocab.txt",
    ],
};

/// NVIDIA Parakeet EOU 120M (ONNX) - Lightweight streaming model
pub static ONNX_PARAKEET_EOU_120M: OnnxChecker = OnnxChecker {
    primary: "realtime_eou_120m-v1-onnx/encoder.onnx",
    required: &[
        "realtime_eou_120m-v1-onnx/encoder.onnx",
        "realtime_eou_120m-v1-onnx/decoder_joint.onnx",
        "realtime_eou_120m-v1-onnx/tokenizer.json",
    ],
};

/// Qwen3-ASR 0.6B (ONNX, int4 decoder) - split init/step decoder with shared
/// external weights + host-side FP16 embedding table.
pub static ONNX_QWEN3_ASR_0_6B: OnnxChecker = OnnxChecker {
    primary: "encoder.int4.onnx",
    required: &[
        "encoder.int4.onnx",
        "decoder_init.int4.onnx",
        "decoder_step.int4.onnx",
        "decoder_weights.int4.data",
        "embed_tokens.bin",
        "tokenizer.json",
        "config.json",
    ],
};

// ============================================================================
// Transformation models - T5-family grammar correctors
// ============================================================================

/// T5-Mini Grammar (visheratin's pre-quantized ONNX export - MIT license, C4_200M training data).
/// We download the no-KV-cache `init_decoder_model_quantized` not the KV-cache `decoder_*`,
/// because our inference path re-runs the decoder fresh per token.
pub static ONNX_T5_MINI_GRAMMAR: OnnxChecker = OnnxChecker {
    primary: "encoder_model_quantized.onnx",
    required: &[
        "encoder_model_quantized.onnx",
        "init_decoder_model_quantized.onnx",
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
        "config.json",
    ],
};

// Unit tests require tempfile crate - integration tests via app verify behavior
