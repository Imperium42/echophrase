// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Maps known backend transcription error markers to friendly locale keys.
 *
 * The Rust ONNX backend fails fast with `MODEL_STILL_LOADING` when a
 * recording arrives while the model is still loading (first-run DirectML
 * shader compile can take minutes). Without this mapping the user saw the raw
 * "Transcription failed: MODEL_STILL_LOADING: ..." text.
 */

/** Mirrors `MODEL_STILL_LOADING_MARKER` in src-tauri/src/transcription/onnx_backend.rs. */
export const MODEL_STILL_LOADING_MARKER = 'MODEL_STILL_LOADING';

/** Locale key for a known backend error, or null to show the raw message. */
export function transcriptionErrorKey(message: string): string | null {
  if (message.includes(MODEL_STILL_LOADING_MARKER)) return 'modelStore.stillLoading';
  return null;
}
