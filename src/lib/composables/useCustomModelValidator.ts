// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Client-side pre-flight validation for BYO custom model imports.
 *
 * Instant extension/size checks before invoking the backend - the Rust
 * command (`import_custom_model_from_file`) is always the authoritative
 * check (it loads the file in ONNX Runtime). Returns `{ isValid, error }`.
 *
 * TODO(safetensors): allow `.safetensors` again once the Candle backend
 * returns (ONNX_ONLY_MODE lifted).
 */
import { translate } from '$lib/locales';

/** Mirror of Rust `MAX_CUSTOM_MODEL_BYTES` (8 GB). */
export const MAX_CUSTOM_MODEL_BYTES = 8 * 1024 * 1024 * 1024;

/** Extensions accepted for import in v1. */
const ACCEPTED_EXTENSIONS = ['onnx'];

export function useCustomModelValidator() {
  /** Validate a file path (extension allowlist only - no filesystem access). */
  function validatePath(path: string): { isValid: boolean; error?: string } {
    const ext = path.split('.').pop()?.toLowerCase() ?? '';
    if (!ACCEPTED_EXTENSIONS.includes(ext)) {
      return { isValid: false, error: translate('customModel.errors.UnsupportedFormat') };
    }
    return { isValid: true };
  }

  /** Validate a known file size against the import cap. */
  function validateSize(sizeBytes: number): { isValid: boolean; error?: string } {
    if (sizeBytes > MAX_CUSTOM_MODEL_BYTES) {
      return { isValid: false, error: translate('customModel.errors.FileTooLarge') };
    }
    return { isValid: true };
  }

  /** Quick shape check for HuggingFace input: `org/repo` or a HF URL. */
  function looksLikeHfRepo(input: string): boolean {
    const trimmed = input.trim();
    if (/^(https?:\/\/)?huggingface\.co\/[^/\s]+\/[^/\s]+/.test(trimmed)) return true;
    return /^[\w.-]+\/[\w.-]+$/.test(trimmed);
  }

  return {
    acceptedExtensions: ACCEPTED_EXTENSIONS,
    maxSize: MAX_CUSTOM_MODEL_BYTES,
    validatePath,
    validateSize,
    looksLikeHfRepo,
  };
}
