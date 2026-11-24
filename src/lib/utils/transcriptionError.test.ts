// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { MODEL_STILL_LOADING_MARKER, transcriptionErrorKey } from './transcriptionError';

describe('transcriptionErrorKey', () => {
  it('maps the backend still-loading marker to the friendly locale key', () => {
    const raw = `Transcription failed: ${MODEL_STILL_LOADING_MARKER}: model load in progress`;
    expect(transcriptionErrorKey(raw)).toBe('modelStore.stillLoading');
  });

  it('matches the marker anywhere in the message', () => {
    expect(transcriptionErrorKey(`Error: x ${MODEL_STILL_LOADING_MARKER}`)).toBe(
      'modelStore.stillLoading'
    );
  });

  it('returns null for other errors so the raw message is still shown', () => {
    expect(transcriptionErrorKey('Transcription failed: decoder error')).toBeNull();
    expect(transcriptionErrorKey('')).toBeNull();
  });

  it('keeps the marker in sync with the Rust constant', () => {
    expect(MODEL_STILL_LOADING_MARKER).toBe('MODEL_STILL_LOADING');
  });
});
