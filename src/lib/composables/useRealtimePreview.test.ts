// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { useRealtimePreview } from './useRealtimePreview';

describe('fadeOutNow (Copy Last hover fades the transcript early)', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('fades a shown final transcript right away instead of after the 10s linger', () => {
    const preview = useRealtimePreview();
    preview.handleSegment('Testing one two three.', true, null);
    expect(get(preview.isVisible)).toBe(true);

    preview.fadeOutNow();
    expect(get(preview.isFading)).toBe(true);
    vi.advanceTimersByTime(500);
    expect(get(preview.isVisible)).toBe(false);
    expect(get(preview.text)).toBe('');
  });

  it('does nothing when no transcript is shown', () => {
    const preview = useRealtimePreview();
    preview.fadeOutNow();
    expect(get(preview.isFading)).toBe(false);
    expect(get(preview.isVisible)).toBe(false);
  });
});
