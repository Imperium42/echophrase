// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Silence detection guard for the transcription router.
//!
//! Qwen3-ASR (and, less often, other backends) hallucinate a small fixed set
//! of tokens when fed (near-)silent audio - observed live 2026-08 as
//! "Current.", "當前。", "None", and concatenations of only those (e.g.
//! "NoneCurrent."). Users see these pasted into whatever they were typing
//! with no indication anything went wrong.
//!
//! Two independent checks feed into [`crate::transcription::transcribe_source`]:
//! 1. Level-based (primary): the captured samples' peak amplitude is below
//!    [`SILENCE_PEAK_THRESHOLD`] - skips the model call entirely.
//! 2. Known-hallucination match (backstop): the model ran anyway (level check
//!    didn't trip, e.g. a brief click or low-level room tone) but returned
//!    only known silence tokens.

/// Peak amplitude (linear, `[0.0, 1.0]`) below which captured audio is
/// treated as silence regardless of what the model would say about it.
/// Tuned against live silent captures that measured peak ~0.0002-0.0008 and
/// RMS ~0.00006 - comfortably below normal room-tone/mic-noise floors
/// (typically peak > 0.005) while still catching the "technically not zero
/// but nothing anyone spoke" case.
pub const SILENCE_PEAK_THRESHOLD: f32 = 0.001;

/// Sentinel returned by [`crate::transcription::transcribe_source`] when
/// silence is detected (either check). Deliberately reuses the literal the
/// frontend already special-cases in `useRecording.ts` /
/// `useTranscriptionDispatch.ts` to skip usage tracking and output routing
/// for empty results, so no new plumbing is needed there - only the display
/// layer needs to map it to a friendlier message.
pub const SILENCE_RESULT: &str = "No speech detected";

/// Known fixed strings Qwen3-ASR hallucinates on (near-)silent input.
/// Provenance: observed directly in live silent-mic captures, 2026-08-04
/// (silent WSLg mic, Qwen3-ASR). Kept as literal strings,
/// not a regex, since the set is small and exact.
const KNOWN_SILENCE_HALLUCINATIONS: &[&str] = &["Current.", "當前。", "None"];

/// True when `text` (already trimmed by the caller ideally, but trimmed here
/// too for safety) is empty, exactly one known hallucination, or a
/// concatenation formed by joining only known hallucinations back-to-back
/// with no other characters in between (e.g. "NoneCurrent.").
pub fn is_known_silence_hallucination(text: &str) -> bool {
    let mut remaining = text.trim();
    if remaining.is_empty() {
        return true;
    }
    'outer: while !remaining.is_empty() {
        for token in KNOWN_SILENCE_HALLUCINATIONS {
            if let Some(rest) = remaining.strip_prefix(token) {
                remaining = rest;
                continue 'outer;
            }
        }
        // No known token matched at the current position - not a pure
        // concatenation of hallucinations.
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_each_known_hallucination_exactly() {
        for token in KNOWN_SILENCE_HALLUCINATIONS {
            assert!(
                is_known_silence_hallucination(token),
                "expected {token:?} to match"
            );
        }
    }

    #[test]
    fn matches_concatenation_of_known_hallucinations() {
        assert!(is_known_silence_hallucination("NoneCurrent."));
        assert!(is_known_silence_hallucination("Current.None當前。"));
        assert!(is_known_silence_hallucination("Current.Current."));
    }

    #[test]
    fn matches_empty_or_whitespace_only() {
        assert!(is_known_silence_hallucination(""));
        assert!(is_known_silence_hallucination("   "));
    }

    #[test]
    fn rejects_real_speech() {
        assert!(!is_known_silence_hallucination("Hello, how are you today?"));
        assert!(!is_known_silence_hallucination(
            "The current weather is nice."
        ));
    }

    #[test]
    fn rejects_hallucination_mixed_with_real_text() {
        assert!(!is_known_silence_hallucination("None of that is true"));
        assert!(!is_known_silence_hallucination("Current. Then I said hi"));
    }

    #[test]
    fn rejects_partial_token_prefix() {
        assert!(!is_known_silence_hallucination("Curr"));
        assert!(!is_known_silence_hallucination("Non"));
    }
}
