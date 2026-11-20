// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { clampScale, computeEffectiveScale, type UiScaleInputs } from './useUiScale';

function inputs(overrides: Partial<UiScaleInputs>): UiScaleInputs {
  return {
    mode: 'auto',
    manual: 1.0,
    withWindow: false,
    windowWidth: null,
    referenceWidth: null,
    ...overrides,
  };
}

describe('clampScale', () => {
  it('clamps to the 0.5-2.0 range', () => {
    expect(clampScale(0.1)).toBe(0.5);
    expect(clampScale(2.5)).toBe(2.0);
    expect(clampScale(1.5)).toBe(1.5);
  });

  it('falls back to 1.0 for invalid values', () => {
    expect(clampScale(NaN)).toBe(1.0);
    expect(clampScale(0)).toBe(1.0);
    expect(clampScale(-2)).toBe(1.0);
  });
});

describe('computeEffectiveScale', () => {
  it('manual mode uses the manual factor (150%)', () => {
    expect(computeEffectiveScale(inputs({ mode: 'manual', manual: 1.5 }))).toBe(1.5);
  });

  it('manual mode clamps out-of-range persisted values', () => {
    expect(computeEffectiveScale(inputs({ mode: 'manual', manual: 3.0 }))).toBe(2.0);
    expect(computeEffectiveScale(inputs({ mode: 'manual', manual: 0.2 }))).toBe(0.5);
  });

  it('auto mode is always 1.0 (webview already renders at native DPI)', () => {
    // Regression guard: never multiply zoom by the OS display scale factor.
    // Doing so double-applied HiDPI scaling and produced a giant window on
    // macOS Retina (scale_factor 2.0) on first launch.
    expect(computeEffectiveScale(inputs({ mode: 'auto' }))).toBe(1.0);
  });

  it('auto mode ignores window size when scale-with-window is off', () => {
    expect(computeEffectiveScale(inputs({ mode: 'auto', windowWidth: 2000 }))).toBe(1.0);
  });

  it('scale-with-window multiplies base by width ratio', () => {
    const result = computeEffectiveScale(
      inputs({
        mode: 'manual',
        manual: 1.0,
        withWindow: true,
        windowWidth: 1200,
        referenceWidth: 800,
      })
    );
    expect(result).toBe(1.5);
  });

  it('scale-with-window result is clamped to 2.0', () => {
    const result = computeEffectiveScale(
      inputs({
        mode: 'manual',
        manual: 1.5,
        withWindow: true,
        windowWidth: 2400,
        referenceWidth: 800,
      })
    );
    expect(result).toBe(2.0);
  });

  it('scale-with-window is ignored without a reference width', () => {
    const result = computeEffectiveScale(
      inputs({ mode: 'manual', manual: 1.2, withWindow: true, windowWidth: 1200 })
    );
    expect(result).toBe(1.2);
  });
});
