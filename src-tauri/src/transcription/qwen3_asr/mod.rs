// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Qwen3-ASR speech recognition backend (Alibaba, Apache-2.0).
//!
//! Multilingual batch transcription: 52 languages including Chinese dialects,
//! with strong EN/ZH code-switching. This is the first `supports_eastern_languages`
//! model in the registry.
//!
//! Architecture (from the andrewleech/qwen3-asr-0.6b-onnx community export,
//! int4 RTN quantized decoder):
//! - `encoder.int4.onnx` - mel spectrogram -> audio features (FP32 weights inlined)
//! - `decoder_init.int4.onnx` - decoder prefill: `input_ids` + audio features -> logits + KV cache
//! - `decoder_step.int4.onnx` - autoregressive step: `input_embeds` + KV cache -> logits
//! - `decoder_weights.int4.data` - shared external weights for both decoders
//! - `embed_tokens.bin` - FP16 embedding matrix, looked up host-side per step
//!
//! The model emits `language <Name>\n<transcription>`; the language prefix is
//! stripped before returning (see `strip_language_prefix`).
//!
//! Inference structure ported from andrewleech/transcribe-rs (MIT), branch
//! `feat/qwen3-batch`, adapted to this codebase's ort session and settings
//! conventions.

mod config;
mod mel;
mod model;
mod prompt;

use anyhow::{Result, anyhow};
use parking_lot::Mutex;
use std::sync::{Arc, LazyLock};

use crate::transcription::backend::TranscriptionBackend;
use model::Qwen3AsrModel;

/// Maximum decoder tokens per chunk. 512 covers ~60 s of dense speech.
const MAX_TOKENS: usize = 512;

/// Qwen3-ASR is trained for utterance-level audio; split longer inputs into
/// chunks of at most this many seconds and transcribe sequentially. One
/// second above the 30 s default recording length, so a default-length
/// recording is never split.
const CHUNK_SECONDS: usize = 31;
/// Preferred chunk length; cuts land on the quietest stretch within
/// `SEARCH_WINDOW_SECONDS` of it so words are not sliced in half.
const TARGET_CHUNK_SECONDS: f32 = 26.0;
const SEARCH_WINDOW_SECONDS: f32 = 5.0;
const SAMPLE_RATE: usize = 16_000;

/// Cached loaded model, shared across calls. Mirrors `CACHED_PARAKEET_TDT`:
/// outer mutex guards the cache slot, `Arc<Mutex<..>>` lets the load lock be
/// released before long-running inference.
static CACHED_QWEN3: LazyLock<Mutex<Option<Arc<Mutex<Qwen3AsrModel>>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Serialises concurrent model loads (same rationale as `LOADING_PARAKEET_TDT`:
/// ORT's DirectML session-creation path is not safe for concurrent loads).
static LOADING_QWEN3: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Transcription backend for Qwen3-ASR registry models.
pub struct Qwen3AsrBackend {
    model_id: String,
}

impl Qwen3AsrBackend {
    pub fn new(model_id: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
        }
    }

    /// Drop the cached model to free memory (registry model switch / unload).
    #[allow(dead_code)]
    pub fn unload() {
        *CACHED_QWEN3.lock() = None;
    }
}

impl TranscriptionBackend for Qwen3AsrBackend {
    async fn transcribe_samples(&self, samples: &[f32]) -> Result<String> {
        let model = get_or_load_model(&self.model_id)?;
        let samples = samples.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut model = model.lock();
            transcribe_chunked(&mut model, &samples)
        })
        .await
        .map_err(|e| anyhow!("Qwen3-ASR task join error: {e}"))?
    }

    fn backend_name(&self) -> &'static str {
        "Qwen3-ASR 0.6B (ONNX)"
    }
}

/// Duration of the silent buffer used to warm the inference path. Long enough
/// to produce real encoder frames, short enough to stay cheap.
const WARMUP_SECONDS: usize = 1;

/// Preload the model so the first transcription doesn't pay the load cost.
///
/// Loading the sessions is not sufficient on its own: ORT defers arena
/// allocation and per-kernel setup to the first `Run()`, and Qwen3-ASR runs
/// three sessions (encoder -> decoder_init -> decoder_step), so the first real
/// transcription paid ~2s even with the model resident. Running one throwaway
/// inference over silence here moves that cost into the preload, where the
/// progress indicator already accounts for it.
#[allow(dead_code)]
pub fn preload(model_id: &str) -> Result<()> {
    let model = get_or_load_model(model_id)?;

    let started = std::time::Instant::now();
    let silence = vec![0.0f32; WARMUP_SECONDS * SAMPLE_RATE];
    // Cap tokens: we only need the decode loop to execute, not to decode a
    // real utterance. A warmup failure is never fatal - the model is loaded
    // and the next real transcription simply pays the old first-run cost.
    match model.lock().transcribe(&silence, 4, None) {
        Ok(_) => log::info!(
            "Qwen3-ASR inference warmed in {:.1}s",
            started.elapsed().as_secs_f32()
        ),
        Err(e) => log::warn!("Qwen3-ASR warmup inference failed (non-fatal): {e}"),
    }

    Ok(())
}

/// Return the cached model, loading it under the serialization lock if needed.
fn get_or_load_model(model_id: &str) -> Result<Arc<Mutex<Qwen3AsrModel>>> {
    if let Some(model) = CACHED_QWEN3.lock().as_ref() {
        return Ok(Arc::clone(model));
    }

    // Serialise loads; a waiter that blocked here re-checks the cache after
    // acquiring the lock and skips the redundant load.
    let _load_guard = LOADING_QWEN3.lock();
    if let Some(model) = CACHED_QWEN3.lock().as_ref() {
        return Ok(Arc::clone(model));
    }

    let entry = crate::models::get_model_info(model_id)
        .ok_or_else(|| anyhow!("Qwen3-ASR model '{model_id}' is not in the registry"))?;
    let model_dir = entry
        .resolve_model_dir()
        .ok_or_else(|| anyhow!("Qwen3-ASR model '{model_id}' is not present on disk"))?;

    let started = std::time::Instant::now();
    let model = Qwen3AsrModel::load(&model_dir)?;
    log::info!(
        "Qwen3-ASR model '{}' loaded in {:.1}s",
        model_id,
        started.elapsed().as_secs_f32()
    );

    let arc = Arc::new(Mutex::new(model));
    *CACHED_QWEN3.lock() = Some(Arc::clone(&arc));
    Ok(arc)
}

/// Fallback hint retried once when auto-language decode collapses to an
/// empty transcript (see the comment in `transcribe_chunked`). Derived from
/// the user's UI locale rather than hardcoded: a zh-TW user whose zero-shot
/// detection hiccups almost certainly spoke Chinese, not English. English
/// remains the default when the locale gives no better signal.
fn auto_detect_fallback_language() -> &'static str {
    match crate::settings::AppSettings::load() {
        Ok(s) if s.ui_locale.trim().starts_with("zh") => "Chinese",
        _ => "English",
    }
}

/// Split audio into <= CHUNK_SECONDS chunks (cut at quiet frames) and join the per-chunk transcripts.
fn transcribe_chunked(model: &mut Qwen3AsrModel, samples: &[f32]) -> Result<String> {
    let language_hint = language_hint_from_settings();
    let ranges = super::chunking::plan_chunks_with(
        samples,
        SAMPLE_RATE as u32,
        TARGET_CHUNK_SECONDS,
        CHUNK_SECONDS as f32,
        SEARCH_WINDOW_SECONDS,
    );
    let mut parts: Vec<String> = Vec::new();

    for range in ranges {
        let chunk = &samples[range];
        let raw = model.transcribe(chunk, MAX_TOKENS, language_hint.as_deref())?;
        let mut text = strip_language_prefix(&raw).trim().to_string();

        // Auto-language mode leaves the assistant prompt prefix empty (see
        // `prompt::build_prompt_ids`), so the model must freely generate its
        // own `language {Name}<asr_text>` preamble before any real
        // transcript token. That zero-shot preamble is measurably less
        // reliable than the forced-hint path: greedy decode can pick EOS
        // before ever emitting the mandatory `<asr_text>` marker, collapsing
        // real speech to a handful of tokens - `model.rs`'s degenerate-output
        // guard (~line 224) correctly detects that and returns "".
        //
        // This was reproduced directly on BOTH a ~2s single-utterance chunk
        // and a >=7s chunk built from the same speech (see
        // `qwen3_asr::tests::e2e_english_fixture` and
        // `long_chunk_auto_language_transcribes`) - it is not a token-budget
        // or encoder-context-length bug: MAX_TOKENS is nowhere near hit
        // (decode stops itself after ~3 tokens), and forcing a language hint
        // into the EXACT SAME audio decodes correctly and quickly. So rather
        // than internally chunking audio (which would not help - length
        // isn't the actual variable) or silently returning nothing for audio
        // the user clearly spoke, retry once with a safe default language
        // hint before giving up.
        if text.is_empty() && language_hint.is_none() {
            let fallback = auto_detect_fallback_language();
            log::warn!(
                "Qwen3-ASR: auto-language decode returned empty for a {:.1}s chunk; \
                 retrying with fallback language {fallback:?}",
                chunk.len() as f32 / SAMPLE_RATE as f32
            );
            let retry_raw = model.transcribe(chunk, MAX_TOKENS, Some(fallback))?;
            text = strip_language_prefix(&retry_raw).trim().to_string();
        }

        if !text.is_empty() {
            parts.push(text);
        }
    }

    Ok(parts.join(" "))
}

/// Resolve the language hint from settings. `auto`/empty means model-side
/// language detection; anything else is normalized ("en" -> "English") and
/// forced into the prompt, which skips detection entirely.
fn language_hint_from_settings() -> Option<String> {
    let language = crate::settings::AppSettings::load().ok()?.language;
    let trimmed = language.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return None;
    }
    Some(normalize_language(trimmed).to_string())
}

/// Normalize a BCP-47 / ISO 639-1 code to the full English name the Qwen3-ASR
/// prompt template expects. Full names and unknown codes pass through.
fn normalize_language(code: &str) -> &str {
    match code {
        "en" => "English",
        "zh" | "zh-Hans" | "zh-Hant" | "zh-TW" | "zh-CN" => "Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "yue" => "Cantonese",
        "fr" => "French",
        "es" => "Spanish",
        "de" => "German",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "ar" => "Arabic",
        "hi" => "Hindi",
        "th" => "Thai",
        "vi" => "Vietnamese",
        "id" => "Indonesian",
        "ms" => "Malay",
        "tr" => "Turkish",
        "nl" => "Dutch",
        "pl" => "Polish",
        "sv" => "Swedish",
        "no" => "Norwegian",
        "da" => "Danish",
        "fi" => "Finnish",
        "cs" => "Czech",
        "ro" => "Romanian",
        "hu" => "Hungarian",
        "el" => "Greek",
        "he" => "Hebrew",
        "uk" => "Ukrainian",
        "bg" => "Bulgarian",
        "hr" => "Croatian",
        "sk" => "Slovak",
        "sl" => "Slovenian",
        "lt" => "Lithuanian",
        "lv" => "Latvian",
        "et" => "Estonian",
        "sr" => "Serbian",
        "tl" | "fil" => "Tagalog",
        "fa" => "Persian",
        "ur" => "Urdu",
        "bn" => "Bengali",
        "ta" => "Tamil",
        "te" => "Telugu",
        "ne" => "Nepali",
        "km" => "Khmer",
        "lo" => "Lao",
        "my" => "Burmese",
        "kk" => "Kazakh",
        "uz" => "Uzbek",
        "az" => "Azerbaijani",
        "ka" => "Georgian",
        "hy" => "Armenian",
        "sw" => "Swahili",
        "am" => "Amharic",
        "ca" => "Catalan",
        "gl" => "Galician",
        "eu" => "Basque",
        "af" => "Afrikaans",
        "mk" => "Macedonian",
        other => other,
    }
}

/// Known language names emitted by Qwen3-ASR as the prefix tag (used for the
/// rare no-newline fallback in `strip_language_prefix`).
const KNOWN_LANGUAGES: &[&str] = &[
    "English",
    "Chinese",
    "Japanese",
    "Korean",
    "Cantonese",
    "French",
    "Spanish",
    "German",
    "Italian",
    "Portuguese",
    "Russian",
    "Arabic",
    "Hindi",
    "Thai",
    "Vietnamese",
    "Indonesian",
    "Malay",
    "Turkish",
    "Dutch",
    "Polish",
    "Swedish",
    "Norwegian",
    "Danish",
    "Finnish",
    "Czech",
    "Romanian",
    "Hungarian",
    "Greek",
    "Hebrew",
    "Ukrainian",
    "Bulgarian",
    "Croatian",
    "Slovak",
    "Slovenian",
    "Lithuanian",
    "Latvian",
    "Estonian",
    "Serbian",
    "Tagalog",
    "Persian",
    "Urdu",
    "Bengali",
    "Tamil",
    "Telugu",
    "Nepali",
    "Khmer",
    "Lao",
    "Burmese",
    "Kazakh",
    "Uzbek",
    "Azerbaijani",
    "Georgian",
    "Armenian",
    "Swahili",
    "Amharic",
    "Catalan",
    "Galician",
    "Basque",
    "Afrikaans",
    "Macedonian",
];

/// Literal `<asr_text>` tag the model emits between the language identification
/// prefix and the transcript body in free-generation (auto-language) mode.
/// Special-token skipping during decode is unreliable for this token id, so we
/// belt-and-suspenders strip it here as well as in `Qwen3AsrModel::decode`.
const ASR_TEXT_TAG: &str = "<asr_text>";

/// Strip the `language <Name>\n` or `language <Name><asr_text>` identification
/// prefix from raw decoder output, and remove any stray `<asr_text>` tag.
///
/// Primary: split on the newline delimiter (handles unknown languages).
/// Fallback: match a known language name when the newline is absent.
/// After removing the language prefix (or if there was none), any remaining
/// `<asr_text>` tag - leading, trailing, or mid-string - is also stripped.
fn strip_language_prefix(text: &str) -> String {
    let without_language = if let Some(rest) = text.strip_prefix("language ") {
        if let Some(newline_pos) = rest.find('\n') {
            rest[newline_pos + 1..].to_string()
        } else if let Some(after) = rest.strip_prefix(ASR_TEXT_TAG) {
            after.to_string()
        } else {
            let mut stripped = None;
            for lang in KNOWN_LANGUAGES {
                if let Some(after) = rest.strip_prefix(lang) {
                    stripped = Some(after.to_string());
                    break;
                }
            }
            match stripped {
                Some(s) => s,
                None => {
                    log::warn!(
                        "Qwen3-ASR: unrecognised language prefix with no newline separator; raw output kept"
                    );
                    rest.to_string()
                }
            }
        }
    } else {
        text.to_string()
    };

    if without_language.contains(ASR_TEXT_TAG) {
        without_language.replace(ASR_TEXT_TAG, "")
    } else {
        without_language
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resolve a fixture under `tests/audio/`, or None when absent.
    fn fixture(name: &str) -> Option<std::path::PathBuf> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .join("tests/audio")
            .join(name);
        path.exists().then_some(path)
    }

    /// Run the full pipeline on a fixture WAV. Returns None (skip) when the
    /// model or fixture is not on disk.
    async fn transcribe_fixture(name: &str) -> Option<String> {
        let entry = crate::models::get_model_info("qwen3-asr-0.6b").expect("registry entry");
        if !entry.is_downloaded() {
            log::debug!("SKIP: qwen3-asr-0.6b not downloaded");
            return None;
        }
        let Some(path) = fixture(name) else {
            log::debug!("SKIP: fixture {name} not present");
            return None;
        };
        let backend = Qwen3AsrBackend::new("qwen3-asr-0.6b");
        let source = echo_audio::AudioSource::File(path);
        let text = backend.transcribe(&source).await.expect("transcription");
        log::debug!("qwen3-asr [{name}] -> {text:?}");
        Some(text)
    }

    /// Real E2E: English fixture ("Test one, two.") through mel -> encoder ->
    /// KV-cache decode -> tokenizer. Ignored by default (needs the ~2GB model):
    /// `cargo test -p echophrase qwen3 --lib -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn e2e_english_fixture() {
        let Some(text) = transcribe_fixture("test-1-2.wav").await else {
            return;
        };
        let lower = text.to_lowercase();
        assert!(
            lower.contains("test") && lower.contains("one") && lower.contains("two"),
            "expected 'test one two' in transcript, got: {text:?}"
        );
    }

    /// Real E2E: Mandarin fixture + OpenCC s2t. Skips unless the local-only
    /// fixture `mandarin-sample.wav` exists (not committed - drop any 16kHz
    /// mono Mandarin WAV in tests/audio/ to run).
    #[tokio::test]
    #[ignore]
    async fn e2e_mandarin_fixture_s2t() {
        let Some(text) = transcribe_fixture("mandarin-sample.wav").await else {
            return;
        };
        assert!(
            crate::transform::opencc::contains_han(&text),
            "expected Han characters in Mandarin transcript, got: {text:?}"
        );
        let traditional = crate::transform::convert_chinese_script(text.clone(), "traditional");
        log::debug!("s2t -> {traditional:?}");
        assert!(crate::transform::opencc::contains_han(&traditional));
    }

    /// Reproduce the open-mic bug report: in auto-language mode, greedy
    /// decode can pick EOS before the mandatory `language {Name}<asr_text>`
    /// preamble is ever emitted, collapsing real speech to a handful of
    /// tokens and an empty transcript (the degenerate-output guard in
    /// `model.rs`, ~line 224, then correctly returns ""). Direct
    /// experimentation (see the root-cause writeup) showed this happens on
    /// BOTH a plain ~2s chunk and a >=7s chunk built from the same speech -
    /// it is not a token-budget or encoder-context-length bug, only whether
    /// a language hint was forced into the prompt. `transcribe_chunked`'s
    /// fallback-to-English retry (this module) is the fix under test here:
    /// go through the full `Qwen3AsrBackend`, not the raw model layer, so the
    /// retry actually runs. Tiling to >=7s matches the field report's
    /// open-mic chunk duration. Ignored by default (needs the ~2GB model):
    /// `cargo test -p echophrase qwen3::tests::long_chunk_auto_language_transcribes --lib -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn long_chunk_auto_language_transcribes() {
        let entry = crate::models::get_model_info("qwen3-asr-0.6b").expect("registry entry");
        if !entry.is_downloaded() {
            log::debug!("SKIP: qwen3-asr-0.6b not downloaded");
            return;
        }
        let Some(path) = fixture("test-1-2.wav") else {
            log::debug!("SKIP: fixture test-1-2.wav not present");
            return;
        };
        let base_raw = echo_audio::wav::read_wav_mono_16k(&path).expect("read fixture wav");
        // Fixture is deliberately quiet (peak ~0.055); normalize like a real
        // captured utterance so we're not confounding "quiet audio" with
        // "long audio" as the cause of degenerate output.
        let base = echo_audio::sample::normalize_peak(&base_raw, echo_audio::NORMALIZE_TARGET_PEAK);

        // Tile the ~2s fixture with 300ms silence gaps until we clear 7.5s,
        // matching the open-mic log's ~7488ms chunk.
        const SAMPLE_RATE: usize = 16_000;
        let gap = vec![0.0f32; SAMPLE_RATE * 3 / 10];
        let mut tiled = Vec::new();
        while tiled.len() < SAMPLE_RATE * 75 / 10 {
            if !tiled.is_empty() {
                tiled.extend_from_slice(&gap);
            }
            tiled.extend_from_slice(&base);
        }
        log::debug!(
            "tiled chunk: {} samples ({:.2}s)",
            tiled.len(),
            tiled.len() as f32 / SAMPLE_RATE as f32
        );

        let backend = Qwen3AsrBackend::new("qwen3-asr-0.6b");
        let source = echo_audio::AudioSource::from_samples_16k(tiled.clone());
        let text = backend.transcribe(&source).await.expect("transcription");
        log::debug!("long-chunk auto-language transcript: {text:?}");
        assert!(
            !text.trim().is_empty(),
            "expected non-empty transcription for a {:.1}s speech chunk, got empty",
            tiled.len() as f32 / SAMPLE_RATE as f32
        );
    }

    #[test]
    fn strips_language_prefix_with_newline() {
        assert_eq!(
            strip_language_prefix("language English\nHello world"),
            "Hello world"
        );
        assert_eq!(strip_language_prefix("language Chinese\n你好"), "你好");
    }

    #[test]
    fn strips_language_prefix_without_newline() {
        assert_eq!(strip_language_prefix("language ChineseNi hao"), "Ni hao");
    }

    #[test]
    fn passthrough_without_prefix() {
        assert_eq!(strip_language_prefix("Hello"), "Hello");
        assert_eq!(strip_language_prefix(""), "");
    }

    #[test]
    fn strips_asr_text_tag_after_language_prefix() {
        assert_eq!(
            strip_language_prefix("language English<asr_text>Hello."),
            "Hello."
        );
    }

    #[test]
    fn strips_bare_asr_text_tag_without_language_prefix() {
        assert_eq!(strip_language_prefix("<asr_text>Hello."), "Hello.");
    }

    #[test]
    fn strips_asr_text_tag_appearing_mid_string() {
        assert_eq!(strip_language_prefix("Hello<asr_text>world"), "Helloworld");
    }

    #[test]
    fn normalizes_language_codes() {
        assert_eq!(normalize_language("en"), "English");
        assert_eq!(normalize_language("zh-TW"), "Chinese");
        assert_eq!(normalize_language("Chinese"), "Chinese");
        assert_eq!(normalize_language("xx"), "xx");
    }
}
