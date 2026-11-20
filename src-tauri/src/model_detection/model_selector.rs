// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Model selection algorithm based on VRAM tier.
//!
//! This module determines the optimal transcription model based on available GPU VRAM.
//! Selection prioritizes quality while ensuring stable performance.

use super::vram_analyzer::VramTier;
use serde::Serialize;

/// Model IDs matching MODEL_REGISTRY in models/mod.rs
#[allow(dead_code)]
pub mod model_ids {
    pub const WHISPER_MEDIUM: &str = "whisper-medium";
    pub const DISTIL_LARGE_V3: &str = "distil-large-v3";
    pub const WHISPER_LARGE_V3_TURBO: &str = "whisper-large-v3-turbo";
    pub const PARAKEET_TDT: &str = "parakeet-tdt-0.6b-v3";
    pub const PARAKEET_EOU: &str = "parakeet-eou-120m";
    pub const QWEN3_ASR: &str = "qwen3-asr-0.6b";
}

/// True when the OS locale is not English (e.g. `zh-TW`, `ja-JP`).
///
/// Used as the auto-detection signal for recommending the multilingual
/// Qwen3-ASR model: a non-English system very likely dictates in a language
/// the Parakeet family (European-only / English-only) cannot transcribe.
pub fn is_non_english_system_locale() -> bool {
    sys_locale::get_locale()
        .map(|locale| locale_is_non_english(&locale))
        // Unknown locale already means "assume English"; neutral locale agrees.
        .unwrap_or(false)
}

/// Decide whether a locale tag expresses a *detected* non-English preference.
///
/// Split out from [`is_non_english_system_locale`] so the tag shapes every OS
/// produces can be tested without a machine in that locale:
/// - Linux/BSD: POSIX form from `LANG`/`LC_ALL` - `en_US.UTF-8`, `zh_TW.UTF-8`,
///   and the neutral `C` / `C.UTF-8` / `POSIX`
/// - Windows: BCP-47 from `GetUserDefaultLocaleName` - `en-US`, `zh-Hant-TW`
/// - macOS: BCP-47 from `NSLocale` - `en-US`, `zh-Hant-TW`, sometimes bare `en`
///
/// Returns `false` for the POSIX neutral locales: they mean NO locale was ever
/// configured (the default on bare WSL and minimal containers), not that the
/// user speaks something other than English. Treating absence-of-preference as
/// a positive non-English signal silently handed every unconfigured Linux box
/// Qwen3 instead of Parakeet.
fn locale_is_non_english(locale: &str) -> bool {
    let locale = locale.to_ascii_lowercase();
    // Matched exactly (plus the "c.<charset>" form) so real language codes
    // beginning with 'c' - ca (Catalan), cs (Czech), cy (Welsh) - still count.
    if matches!(locale.as_str(), "c" | "posix") || locale.starts_with("c.") {
        return false;
    }
    // Region/charset suffixes are irrelevant: en_US.UTF-8, en-GB and bare "en"
    // are all English. Guard the separator so a hypothetical "eng"-prefixed
    // non-English tag isn't swallowed by a bare starts_with("en").
    locale != "en" && !locale.starts_with("en-") && !locale.starts_with("en_")
}

/// Whether the OS locale indicates a Simplified-Chinese-writing user, so first
/// launch can default the Chinese script setting to Simplified instead of the
/// global Traditional default.
pub fn system_prefers_simplified_chinese() -> bool {
    sys_locale::get_locale()
        .map(|locale| locale_prefers_simplified_chinese(&locale))
        .unwrap_or(false)
}

/// Pure form of [`system_prefers_simplified_chinese`], testable against each
/// OS's tag shape (POSIX `zh_CN.UTF-8`, BCP-47 `zh-Hans-CN` / `zh-CN`).
///
/// Only Simplified is matched: Traditional is already the global default, so
/// anything unrecognised (`zh-TW`, `zh-Hant`, `zh-HK`, non-Chinese locales)
/// simply falls through and keeps it. That makes this a strict allowlist -
/// a locale we fail to classify degrades to today's behaviour, never to the
/// wrong script.
///
/// Simplified-writing regions: mainland China, Singapore and Malaysia, plus
/// the explicit `Hans` script subtag.
fn locale_prefers_simplified_chinese(locale: &str) -> bool {
    let locale = locale.to_ascii_lowercase();
    if !locale.starts_with("zh") {
        return false;
    }
    // An explicit script subtag always wins over the region. This ordering is
    // load-bearing: a Traditional-writing Singaporean reports "zh-Hant-SG",
    // which the bare "zh-sg" region rule below would otherwise flip.
    if locale.contains("hans") {
        return true;
    }
    if locale.contains("hant") {
        return false;
    }
    // Region-only forms, both separators: zh-cn / zh_CN.UTF-8, sg, my.
    ["cn", "sg", "my"].iter().any(|region| {
        locale.starts_with(&format!("zh-{}", region))
            || locale.starts_with(&format!("zh_{}", region))
    })
}

/// Check if a model ID requires ONNX Runtime.
/// Parakeet models and explicit ONNX models need ORT to run.
#[allow(dead_code)]
fn is_onnx_model_id(model_id: &str) -> bool {
    model_id.starts_with("parakeet-") || model_id.contains("-onnx") || model_id.ends_with(".onnx")
}

/// Result of model auto-detection.
#[derive(Debug, Clone, Serialize)]
pub struct ModelSelection {
    /// Selected model ID
    pub model_id: String,
    /// VRAM tier that was detected
    pub vram_tier: VramTier,
    /// Human-readable reason for selection
    pub reason: String,
    /// Detected VRAM in MB (if available)
    pub vram_mb: Option<u32>,
}

/// Select the optimal model based on available VRAM and runtime availability.
///
/// When ONNX Runtime is available (all platforms except macOS < 13.3), Parakeet is
/// preferred because it supports 25 European languages with auto-detection and
/// runs on any GPU via DirectML (Windows) or CUDA (Linux/Windows with addon).
/// Falls back to Candle/Whisper models when ORT is unavailable.
///
/// Selection priority with ORT available:
/// 1. `parakeet-tdt-0.6b-v3` - 600M params, multilingual (High/Premium tiers, 6GB+)
/// 2. `parakeet-eou-120m` - 120M params, English only, runs on CPU (Low/None tiers)
/// 3. `distil-large-v3` - Candle fallback (Medium tier, needs 4GB for stable ORT)
///
/// Selection priority without ORT (macOS < 13.3):
/// 1. `whisper-large-v3-turbo` - Maximum quality (Premium, 8GB+)
/// 2. `distil-large-v3` - Best speed/quality balance (all other tiers)
///
/// # Arguments
/// * `vram_mb` - Available VRAM in megabytes, None if unknown
/// * `ort_available` - Whether ONNX Runtime can load on this system.
/// * `non_english_locale` - OS locale is not English (see
///   [`is_non_english_system_locale`]). With a capable GPU (4GB+), this
///   selects the multilingual Qwen3-ASR over the Parakeet family.
///
/// # Returns
/// `ModelSelection` with chosen model and reasoning
pub fn select_optimal_model(
    vram_mb: Option<u32>,
    ort_available: bool,
    non_english_locale: bool,
) -> ModelSelection {
    let tier = VramTier::from_vram_mb(vram_mb);

    // Non-English system + GPU with enough VRAM: prefer Qwen3-ASR (52
    // languages incl. Chinese/Japanese/Korean). The int4 build fits in ~4GB;
    // below that tier the CPU-friendly picks below stay in place.
    if ort_available
        && non_english_locale
        && matches!(tier, VramTier::Medium | VramTier::High | VramTier::Premium)
    {
        return ModelSelection {
            model_id: model_ids::QWEN3_ASR.to_string(),
            vram_tier: tier,
            reason: format!(
                "{} MB VRAM + non-English system - Qwen3-ASR 0.6B (52 languages incl. Chinese)",
                vram_mb.unwrap_or(0)
            ),
            vram_mb,
        };
    }

    let (model_id, reason) = if ort_available {
        // ONNX path: Parakeet only. The frontend ONNX_ONLY_MODE flag filters
        // Candle models out of the UI, so writing a Candle model ID here
        // would create a mismatch. Prefer Parakeet TDT 0.6B for GPU users
        // with enough VRAM, fall back to Parakeet Lite 120M everywhere else.
        // AMD GPUs on Windows get DirectML acceleration automatically.
        match tier {
            VramTier::None => (
                model_ids::PARAKEET_EOU,
                "No GPU detected - using Parakeet Lite 120M (runs on CPU)".to_string(),
            ),
            VramTier::Insufficient => (
                model_ids::PARAKEET_EOU,
                format!(
                    "Limited VRAM ({} MB) - using Parakeet Lite 120M",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::Low => (
                model_ids::PARAKEET_EOU,
                format!(
                    "{} MB VRAM - Parakeet Lite 120M for stable performance",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::Medium => (
                // Parakeet TDT 0.6B needs ~6GB, too tight for 4-6GB tier.
                // Fall back to Parakeet Lite (the only other ONNX option).
                model_ids::PARAKEET_EOU,
                format!(
                    "{} MB VRAM - Parakeet Lite 120M (Parakeet TDT needs 6GB+)",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::High => (
                model_ids::PARAKEET_TDT,
                format!(
                    "{} MB VRAM - Parakeet TDT 0.6B (25 languages, highest accuracy)",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::Premium => (
                model_ids::PARAKEET_TDT,
                format!(
                    "{} MB VRAM - Parakeet TDT 0.6B (25 languages, highest accuracy)",
                    vram_mb.unwrap_or(0)
                ),
            ),
        }
    } else {
        // Candle/Whisper fallback when ORT is unavailable (e.g., macOS < 13.3)
        match tier {
            VramTier::None => (
                model_ids::DISTIL_LARGE_V3,
                "No GPU detected - using distil-large-v3 (smallest available model)".to_string(),
            ),
            VramTier::Insufficient => (
                model_ids::DISTIL_LARGE_V3,
                format!(
                    "Limited VRAM ({} MB) - using distil-large-v3",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::Low => (
                model_ids::DISTIL_LARGE_V3,
                format!(
                    "{} MB VRAM detected - distil-large-v3 for stable performance",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::Medium => (
                model_ids::DISTIL_LARGE_V3,
                format!(
                    "{} MB VRAM detected - distil-large-v3 offers best speed/quality balance",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::High => (
                model_ids::DISTIL_LARGE_V3,
                format!(
                    "{} MB VRAM detected - distil-large-v3 recommended (turbo available in settings)",
                    vram_mb.unwrap_or(0)
                ),
            ),
            VramTier::Premium => (
                model_ids::WHISPER_LARGE_V3_TURBO,
                format!(
                    "{} MB VRAM detected - whisper-large-v3-turbo for maximum quality",
                    vram_mb.unwrap_or(0)
                ),
            ),
        }
    };

    ModelSelection {
        model_id: model_id.to_string(),
        vram_tier: tier,
        reason,
        vram_mb,
    }
}

/// Get a list of compatible models for a given VRAM amount.
///
/// Returns models that can run without OOM on the detected VRAM.
/// When `ort_available` is false, ONNX models (parakeet-*) are excluded.
#[allow(dead_code)]
pub fn get_compatible_models(vram_mb: Option<u32>, ort_available: bool) -> Vec<&'static str> {
    let tier = VramTier::from_vram_mb(vram_mb);

    let models = match tier {
        VramTier::None | VramTier::Insufficient | VramTier::Low => {
            vec![model_ids::DISTIL_LARGE_V3]
        }
        VramTier::Medium => {
            vec![model_ids::DISTIL_LARGE_V3, model_ids::WHISPER_MEDIUM]
        }
        VramTier::High => {
            vec![
                model_ids::DISTIL_LARGE_V3,
                model_ids::WHISPER_LARGE_V3_TURBO,
                model_ids::WHISPER_MEDIUM,
                model_ids::PARAKEET_TDT,
            ]
        }
        VramTier::Premium => {
            vec![
                model_ids::WHISPER_LARGE_V3_TURBO,
                model_ids::DISTIL_LARGE_V3,
                model_ids::PARAKEET_TDT,
                model_ids::WHISPER_MEDIUM,
            ]
        }
    };

    if ort_available {
        models
    } else {
        models
            .into_iter()
            .filter(|id| !is_onnx_model_id(id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_no_gpu() {
        let result = select_optimal_model(None, true, false);
        assert_eq!(result.model_id, model_ids::PARAKEET_EOU);
        assert_eq!(result.vram_tier, VramTier::None);
        assert!(result.reason.contains("No GPU"));
    }

    #[test]
    fn test_select_insufficient_vram() {
        let result = select_optimal_model(Some(1500), true, false);
        assert_eq!(result.model_id, model_ids::PARAKEET_EOU);
        assert_eq!(result.vram_tier, VramTier::Insufficient);
    }

    #[test]
    fn test_select_low_vram() {
        let result = select_optimal_model(Some(3072), true, false);
        assert_eq!(result.model_id, model_ids::PARAKEET_EOU);
        assert_eq!(result.vram_tier, VramTier::Low);
    }

    #[test]
    fn test_select_medium_vram() {
        // 4-6GB tier: Parakeet TDT 0.6B needs 6GB minimum, falls back to
        // Parakeet Lite (the other ONNX option in ONNX_ONLY_MODE).
        let result = select_optimal_model(Some(4096), true, false);
        assert_eq!(result.model_id, model_ids::PARAKEET_EOU);
        assert_eq!(result.vram_tier, VramTier::Medium);
    }

    #[test]
    fn test_select_high_vram() {
        let result = select_optimal_model(Some(6144), true, false);
        assert_eq!(result.model_id, model_ids::PARAKEET_TDT);
        assert_eq!(result.vram_tier, VramTier::High);
    }

    #[test]
    fn test_select_premium_vram() {
        let result = select_optimal_model(Some(8192), true, false);
        assert_eq!(result.model_id, model_ids::PARAKEET_TDT);
        assert_eq!(result.vram_tier, VramTier::Premium);

        // Also test 24GB - still Parakeet TDT, not Whisper Turbo
        let result = select_optimal_model(Some(24576), true, false);
        assert_eq!(result.model_id, model_ids::PARAKEET_TDT);
    }

    #[test]
    fn test_select_premium_vram_no_ort() {
        // Without ORT: Whisper Turbo is the highest-quality fallback
        let result = select_optimal_model(Some(8192), false, false);
        assert_eq!(result.model_id, model_ids::WHISPER_LARGE_V3_TURBO);
        assert_eq!(result.vram_tier, VramTier::Premium);
    }

    #[test]
    fn test_compatible_models_low() {
        let models = get_compatible_models(Some(3000), true);
        assert_eq!(models.len(), 1);
        assert!(models.contains(&model_ids::DISTIL_LARGE_V3));
    }

    #[test]
    fn test_compatible_models_premium() {
        let models = get_compatible_models(Some(12000), true);
        assert!(models.len() >= 3);
        assert!(models.contains(&model_ids::WHISPER_LARGE_V3_TURBO));
        assert!(models.contains(&model_ids::DISTIL_LARGE_V3));
    }

    #[test]
    fn test_select_no_gpu_no_ort() {
        // macOS < 13.3 with no discrete GPU - should pick distil-large-v3
        let result = select_optimal_model(None, false, false);
        assert_eq!(result.model_id, model_ids::DISTIL_LARGE_V3);
        assert!(result.reason.contains("No GPU"));
    }

    #[test]
    fn test_compatible_models_premium_no_ort() {
        // High VRAM but ORT unavailable - parakeet should be filtered out
        let models = get_compatible_models(Some(12000), false);
        assert!(!models.contains(&model_ids::PARAKEET_TDT));
        assert!(models.contains(&model_ids::WHISPER_LARGE_V3_TURBO));
        assert!(models.contains(&model_ids::DISTIL_LARGE_V3));
    }

    /// Locale tag shapes differ per OS (POSIX `zh_TW.UTF-8` vs BCP-47
    /// `zh-Hant-TW`), so the classifier is checked against all of them.
    #[test]
    fn test_locale_is_non_english_across_os_tag_formats() {
        // English - every OS's shape, plus region/charset suffixes.
        for tag in [
            "en",          // macOS bare
            "en-US",       // Windows / macOS BCP-47
            "en-GB",       //
            "en_US",       // POSIX
            "en_US.UTF-8", // POSIX with charset
            "EN_US.UTF-8", // case-insensitive
        ] {
            assert!(
                !locale_is_non_english(tag),
                "'{}' should be treated as English",
                tag
            );
        }

        // Genuinely non-English - these SHOULD select Qwen3.
        for tag in [
            "zh-Hant-TW",  // Windows / macOS
            "zh_TW.UTF-8", // POSIX
            "zh-CN",
            "ja-JP",
            "ko_KR.UTF-8",
            "ca-ES", // Catalan: starts with 'c' but is a real language
            "cs_CZ.UTF-8",
            "cy-GB",
        ] {
            assert!(
                locale_is_non_english(tag),
                "'{}' should be treated as non-English",
                tag
            );
        }

        // POSIX neutral locale = no preference configured, NOT non-English.
        // Default on bare WSL and minimal containers.
        for tag in ["C", "c", "C.UTF-8", "c.utf-8", "POSIX", "posix"] {
            assert!(
                !locale_is_non_english(tag),
                "neutral locale '{}' must not count as a non-English preference",
                tag
            );
        }
    }

    #[test]
    fn test_locale_prefers_simplified_chinese() {
        // Simplified-writing regions + explicit Hans script subtag.
        for tag in [
            "zh-CN",
            "zh_CN.UTF-8",
            "zh-SG",
            "zh_SG.UTF-8",
            "zh-MY",
            "zh_MY.UTF-8",
            "zh-Hans",
            "zh-Hans-CN",
            "zh-hans-sg",
            "ZH-CN",
        ] {
            assert!(
                locale_prefers_simplified_chinese(tag),
                "'{}' should default to Simplified",
                tag
            );
        }

        // Traditional-writing Chinese locales keep the Traditional default.
        // "zh-Hant-SG" is the important one: an explicit Traditional script
        // subtag must beat the bare "zh-SG" region rule.
        for tag in [
            "zh-TW",
            "zh_TW.UTF-8",
            "zh-Hant",
            "zh-Hant-HK",
            "zh-HK",
            "zh-Hant-SG",
        ] {
            assert!(
                !locale_prefers_simplified_chinese(tag),
                "'{}' must keep the Traditional default",
                tag
            );
        }

        // Non-Chinese locales are unaffected.
        for tag in ["en-US", "ja-JP", "ko_KR.UTF-8", "C.UTF-8", "posix"] {
            assert!(
                !locale_prefers_simplified_chinese(tag),
                "'{}' is not Chinese and must not flip the script setting",
                tag
            );
        }
    }

    #[test]
    fn test_select_non_english_locale_with_gpu_prefers_qwen3() {
        // 4GB+ GPU + non-English system: Qwen3-ASR at every capable tier.
        for vram in [4096, 6144, 8192, 24576] {
            let result = select_optimal_model(Some(vram), true, true);
            assert_eq!(result.model_id, model_ids::QWEN3_ASR, "vram={vram}");
            assert!(result.reason.contains("non-English"));
        }
    }

    #[test]
    fn test_select_non_english_locale_without_gpu_keeps_default() {
        // No/weak GPU: Qwen3 is too heavy, keep the CPU-friendly pick.
        let result = select_optimal_model(None, true, true);
        assert_eq!(result.model_id, model_ids::PARAKEET_EOU);
        let result = select_optimal_model(Some(3072), true, true);
        assert_eq!(result.model_id, model_ids::PARAKEET_EOU);
    }

    #[test]
    fn test_select_non_english_locale_without_ort_keeps_candle() {
        // ORT unavailable (macOS < 13.3): Qwen3 can't run, Candle fallback.
        let result = select_optimal_model(Some(8192), false, true);
        assert_eq!(result.model_id, model_ids::WHISPER_LARGE_V3_TURBO);
    }

    #[test]
    fn test_is_onnx_model_id() {
        assert!(is_onnx_model_id("parakeet-tdt-0.6b-v3"));
        assert!(is_onnx_model_id("parakeet-eou-120m"));
        assert!(is_onnx_model_id("whisper-large-v3-onnx"));
        assert!(is_onnx_model_id("model.onnx"));
        assert!(!is_onnx_model_id("whisper-medium"));
        assert!(!is_onnx_model_id("distil-large-v3"));
    }
}
