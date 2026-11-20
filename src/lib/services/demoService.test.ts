// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  runDemo,
  cancelDemo,
  isDemoRunning,
  DEMO_RECORD_MS,
  DEMO_CHIPS_PINNED_MS,
  DEFAULT_DEMO_OPTIONS,
  sanitizeDemoOptions,
  DEMO_TEXT,
} from './demoService';
import { demoRecording, demoChipsPinned, demoSession, demoCountdown } from '$lib/stores/demo';
import { lastRun } from '$lib/stores/lastRun';

const CURSOR_SELECTOR = 'div[aria-hidden="true"]';

describe('demoService', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = '<button class="record-btn"></button>';
  });

  afterEach(() => {
    cancelDemo();
    vi.useRealTimers();
    lastRun.clear();
  });

  it('waits 10s, records for 2s, then publishes the mock result', async () => {
    const previews: string[] = [];
    window.addEventListener('transcription-preview', e =>
      previews.push((e as CustomEvent).detail.text)
    );

    const done = runDemo();
    expect(isDemoRunning()).toBe(true);

    expect(get(demoSession)).toBe(true);
    expect(get(demoCountdown)).toBe(10);
    await vi.advanceTimersByTimeAsync(7_000);
    expect(get(demoCountdown)).toBe(3);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(get(demoCountdown)).toBeNull(); // 2 is hidden
    await vi.advanceTimersByTimeAsync(2_000 + 3_000 - 1); // rest of countdown + move grace
    expect(get(demoRecording)).toBe(false);
    expect(document.querySelectorAll(CURSOR_SELECTOR).length).toBe(0);

    // Cursor appears, glides, clicks -> recording flips on.
    await vi.advanceTimersByTimeAsync(3_000);
    expect(get(demoRecording)).toBe(true);

    await vi.advanceTimersByTimeAsync(DEMO_RECORD_MS + 3_000);
    expect(get(demoRecording)).toBe(false);
    expect(previews).toEqual([DEMO_TEXT]);
    expect(get(lastRun)?.speed?.transcriptionSeconds).toBe(0.0001);
    expect(get(demoChipsPinned)).toBe(true);

    await vi.advanceTimersByTimeAsync(DEMO_CHIPS_PINNED_MS + 1_000);
    expect(get(demoSession)).toBe(true); // mock lingers through the grace period
    await vi.advanceTimersByTimeAsync(DEFAULT_DEMO_OPTIONS.resetGraceSec * 1000);
    await done;
    expect(get(demoSession)).toBe(false);
    expect(isDemoRunning()).toBe(false);
    expect(get(demoChipsPinned)).toBe(false);
    expect(get(lastRun)).toBeNull();
    expect(document.querySelectorAll(CURSOR_SELECTOR).length).toBe(0);
  });

  it('sanitizes options', () => {
    expect(sanitizeDemoOptions(null)).toEqual(DEFAULT_DEMO_OPTIONS);
    expect(
      sanitizeDemoOptions({
        startDelaySec: '5',
        moveDelaySec: -1,
        recordSec: 9999,
        resetGraceSec: 9999,
        transcriptionText: '  ',
      })
    ).toEqual({
      startDelaySec: 5,
      moveDelaySec: DEFAULT_DEMO_OPTIONS.moveDelaySec,
      recordSec: DEFAULT_DEMO_OPTIONS.recordSec,
      resetGraceSec: DEFAULT_DEMO_OPTIONS.resetGraceSec,
      transcriptionText: DEFAULT_DEMO_OPTIONS.transcriptionText,
    });
  });

  it('ignores a second start while running', async () => {
    const first = runDemo();
    const second = runDemo();
    await second; // returns immediately
    expect(isDemoRunning()).toBe(true);
    cancelDemo();
    await first;
  });

  it('cancel tears down cursor and state mid-run', async () => {
    const done = runDemo();
    await vi.advanceTimersByTimeAsync(13_000 + 1_500);
    expect(document.querySelectorAll(CURSOR_SELECTOR).length).toBeGreaterThan(0);
    cancelDemo();
    await vi.advanceTimersByTimeAsync(2_000); // drain the in-flight glide fallback timer
    await done;
    expect(isDemoRunning()).toBe(false);
    expect(get(demoRecording)).toBe(false);
    expect(document.querySelectorAll(CURSOR_SELECTOR).length).toBe(0);
  });
});
