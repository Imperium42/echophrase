// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('hotkeys-js', () => ({ default: Object.assign(vi.fn(), { unbind: vi.fn() }) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('$lib/services/globalShortcutService', () => ({}));

import { createHoldHandlers, HYBRID_HOLD_MS, type HotkeyMode } from './useHotkeys';

/**
 * Simulates the real toggle: each callback flips recording on/off, as
 * handleRecordClick -> toggleRecording does in the app.
 */
function setup(mode: HotkeyMode) {
  const state = { recording: false, t: 0 };
  const callback = vi.fn(() => {
    state.recording = !state.recording;
  });
  const handlers = createHoldHandlers(
    callback,
    { mode: () => mode, isRecording: () => state.recording },
    () => state.t
  );
  return { state, callback, ...handlers };
}

describe('createHoldHandlers', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  describe('hybrid (default)', () => {
    it('short tap starts and keeps recording; next tap stops', () => {
      const h = setup('hybrid');
      h.onPress();
      h.state.t = 150;
      h.onRelease();
      expect(h.state.recording).toBe(true);
      h.state.t = 3000;
      h.onPress();
      h.onRelease();
      expect(h.state.recording).toBe(false);
      expect(h.callback).toHaveBeenCalledTimes(2);
    });

    it('long hold stops on release', () => {
      const h = setup('hybrid');
      h.onPress();
      h.onPress(); // OS key repeat
      h.onPress();
      h.state.t = HYBRID_HOLD_MS + 1;
      h.onRelease();
      expect(h.state.recording).toBe(false);
      expect(h.callback).toHaveBeenCalledTimes(2);
    });

    it('long hold released before recorder startup still stops', () => {
      const state = { recording: false, t: 0 };
      const callback = vi.fn();
      const { onPress, onRelease } = createHoldHandlers(
        callback,
        { mode: () => 'hybrid', isRecording: () => state.recording },
        () => state.t
      );
      onPress(); // start requested, recorder not up yet
      state.t = HYBRID_HOLD_MS + 50;
      onRelease();
      expect(callback).toHaveBeenCalledTimes(1);
      state.recording = true; // recorder finally started
      vi.advanceTimersByTime(150);
      expect(callback).toHaveBeenCalledTimes(2);
    });
  });

  describe('hold', () => {
    it('any release stops, even a short tap', () => {
      const h = setup('hold');
      h.onPress();
      h.state.t = 50;
      h.onRelease();
      expect(h.state.recording).toBe(false);
      expect(h.callback).toHaveBeenCalledTimes(2);
    });

    it('release without a matching press is ignored', () => {
      const h = setup('hold');
      h.state.recording = true;
      h.onRelease();
      expect(h.callback).not.toHaveBeenCalled();
    });
  });

  describe('toggle', () => {
    it('every press toggles; release does nothing', () => {
      const h = setup('toggle');
      h.onPress();
      h.state.t = 5000;
      h.onRelease();
      expect(h.state.recording).toBe(true);
      h.onPress();
      expect(h.state.recording).toBe(false);
    });
  });

  it('no options: plain toggle', () => {
    const callback = vi.fn();
    const { onPress, onRelease } = createHoldHandlers(callback);
    onPress();
    onRelease();
    expect(callback).toHaveBeenCalledTimes(1);
  });
});
