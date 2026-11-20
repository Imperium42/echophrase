// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { resolveEmptyStateHint, type EmptyStateHintInput } from './emptyStateHint';

/** Baseline: idle app, onboarded, model present, nothing transcribed yet. */
const ready: EmptyStateHintInput = {
  isRecording: false,
  isProcessing: false,
  isPreloading: false,
  onboardingComplete: true,
  hasTranscribedThisSession: false,
  modelExists: true,
  hotkeyReady: true,
};

describe('resolveEmptyStateHint', () => {
  it('shows the press-to-start hint when idle and ready', () => {
    expect(resolveEmptyStateHint(ready)).toBe('pressToStart');
  });

  it('hides while recording', () => {
    expect(resolveEmptyStateHint({ ...ready, isRecording: true })).toBe('none');
  });

  it('hides while processing', () => {
    expect(resolveEmptyStateHint({ ...ready, isProcessing: true })).toBe('none');
  });

  it('hides while the model is preloading', () => {
    expect(resolveEmptyStateHint({ ...ready, isPreloading: true })).toBe('none');
  });

  it('hides until onboarding is complete', () => {
    expect(resolveEmptyStateHint({ ...ready, onboardingComplete: false })).toBe('none');
  });

  it('hides for the rest of the session once something was transcribed', () => {
    expect(resolveEmptyStateHint({ ...ready, hasTranscribedThisSession: true })).toBe('none');
  });

  it('asks to download a model when none exists', () => {
    expect(resolveEmptyStateHint({ ...ready, modelExists: false })).toBe('downloadModel');
  });

  it('download hint wins over the hotkey placeholder guard', () => {
    expect(resolveEmptyStateHint({ ...ready, modelExists: false, hotkeyReady: false })).toBe(
      'downloadModel'
    );
  });

  it('hides press-to-start until the hotkey store has loaded (no placeholder flash)', () => {
    expect(resolveEmptyStateHint({ ...ready, hotkeyReady: false })).toBe('none');
  });
});
