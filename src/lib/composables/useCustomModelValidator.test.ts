// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { useCustomModelValidator, MAX_CUSTOM_MODEL_BYTES } from './useCustomModelValidator';
import { parseCustomModelError } from '$lib/services/customModelService';

describe('useCustomModelValidator', () => {
  const validator = useCustomModelValidator();

  it('accepts .onnx paths', () => {
    expect(validator.validatePath('/models/whisper.onnx').isValid).toBe(true);
    expect(validator.validatePath('C:\\models\\Model.ONNX').isValid).toBe(true);
  });

  it('rejects non-onnx extensions including safetensors (parked)', () => {
    expect(validator.validatePath('/models/model.safetensors').isValid).toBe(false);
    expect(validator.validatePath('/models/model.gguf').isValid).toBe(false);
    expect(validator.validatePath('/models/noext').isValid).toBe(false);
  });

  it('enforces the size cap', () => {
    expect(validator.validateSize(MAX_CUSTOM_MODEL_BYTES).isValid).toBe(true);
    expect(validator.validateSize(MAX_CUSTOM_MODEL_BYTES + 1).isValid).toBe(false);
  });

  it('recognizes HuggingFace repo inputs', () => {
    expect(validator.looksLikeHfRepo('org/repo')).toBe(true);
    expect(validator.looksLikeHfRepo('https://huggingface.co/org/repo')).toBe(true);
    expect(validator.looksLikeHfRepo('huggingface.co/org/repo/tree/main')).toBe(true);
    expect(validator.looksLikeHfRepo('not a repo')).toBe(false);
    expect(validator.looksLikeHfRepo('https://evil.example/org/repo')).toBe(false);
  });
});

describe('parseCustomModelError', () => {
  it('parses JSON-encoded typed errors from Rust', () => {
    const parsed = parseCustomModelError('{"code":"FileTooLarge","message":"too big"}');
    expect(parsed.code).toBe('FileTooLarge');
    expect(parsed.message).toBe('too big');
  });

  it('wraps unparseable strings as Internal', () => {
    const parsed = parseCustomModelError('boom');
    expect(parsed.code).toBe('Internal');
    expect(parsed.message).toBe('boom');
  });

  it('wraps Error instances as Internal', () => {
    const parsed = parseCustomModelError(new Error('nope'));
    expect(parsed.code).toBe('Internal');
    expect(parsed.message).toBe('nope');
  });
});
