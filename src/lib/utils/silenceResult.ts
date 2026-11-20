// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Silence result helpers
 *
 * The Rust transcription router (`transcribe_source` in
 * `src-tauri/src/transcription/silence.rs`) returns a fixed sentinel string
 * when captured audio is (near-)silent, or when the ASR model hallucinated
 * one of a small set of known fixed strings on near-silent input (observed:
 * "Current.", "當前。", "None"). The sentinel is deliberately the same
 * literal ('No speech detected') that useRecording.ts /
 * useTranscriptionDispatch.ts already special-case to skip usage tracking
 * and output routing - this module is the single place that decides whether
 * a result IS that sentinel, so callers don't duplicate the string literal.
 */

/** Sentinel text returned by the backend for a silent/hallucinated result. */
export const SILENCE_RESULT_SENTINEL = 'No speech detected';

/** True when `text` is the backend's silence sentinel. */
export function isSilenceResult(text: string | null | undefined): boolean {
  return text === SILENCE_RESULT_SENTINEL;
}
