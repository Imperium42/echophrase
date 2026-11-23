// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('hotkeys-js', () => ({ default: Object.assign(vi.fn(), { unbind: vi.fn() }) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('$lib/services/globalShortcutService', () => ({}));

import { createHoldHandlers, HYBRID_HOLD_MS, isMainKeyEvent, type HotkeyMode } from './useHotkeys';

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

  describe('start pipeline (slow mic startup)', () => {
    /** Mimics useRecording: a dispatched start stays pending until `finishStart`. */
    function setupPipeline(mode: HotkeyMode) {
      const st = { recording: false, pending: false, seq: 0, t: 0, startFails: false };
      const callback = vi.fn(() => {
        if (st.pending) return;
        if (st.recording) {
          st.recording = false;
          return;
        }
        st.pending = true;
        st.seq++;
      });
      const finishStart = () => {
        st.pending = false;
        if (!st.startFails) st.recording = true;
      };
      const h = createHoldHandlers(
        callback,
        {
          mode: () => mode,
          isRecording: () => st.recording,
          start: { isPending: () => st.pending, seq: () => st.seq },
        },
        () => st.t
      );
      return { st, callback, finishStart, ...h };
    }

    it('a press before startup completes means stop-once-started, not start again', () => {
      const h = setupPipeline('hybrid');
      h.onPress(); // first tap: start pending
      h.onRelease(); // short tap
      h.onPress(); // second tap while the mic is still opening
      h.onRelease();
      expect(h.st.seq).toBe(1); // only one start dispatched
      h.finishStart();
      vi.advanceTimersByTime(150);
      expect(h.st.recording).toBe(false); // started, then stopped once
    });

    it('a stale release poll cannot stop a recording started later by a click', () => {
      const h = setupPipeline('hold');
      h.st.startFails = true;
      h.onPress();
      h.onRelease(); // poll armed while our start is pending
      h.finishStart(); // our start fails: not recording, nothing pending
      // later (within the old 3s poll window) a click starts a different session
      h.st.startFails = false;
      h.st.pending = true;
      h.st.seq++;
      h.finishStart();
      vi.advanceTimersByTime(500);
      expect(h.st.recording).toBe(true);
    });

    it('a stale poll does not stop a manual start that races our pending start', () => {
      const h = setupPipeline('hold');
      h.onPress();
      h.onRelease();
      h.st.seq++; // a newer start superseded ours
      h.st.pending = false;
      h.st.recording = true;
      vi.advanceTimersByTime(500);
      expect(h.st.recording).toBe(true);
    });
  });

  describe('lost release recovery', () => {
    it('reset() treats a lost keyup as a release and clears held', () => {
      const h = setup('hold');
      h.onPress();
      expect(h.state.recording).toBe(true);
      h.reset(); // window blur / tab hidden
      expect(h.state.recording).toBe(false);
      h.onPress(); // a fresh press works instead of being swallowed as key repeat
      expect(h.state.recording).toBe(true);
    });

    it('reset() with nothing held is a no-op', () => {
      const h = setup('hold');
      h.state.recording = true;
      h.reset();
      expect(h.callback).not.toHaveBeenCalled();
    });
  });

  describe('isMainKeyEvent', () => {
    it('matches the last token, spacebar and code fallbacks', () => {
      expect(isMainKeyEvent({ key: 'A', code: 'KeyA' }, 'ctrl+alt+a')).toBe(true);
      expect(isMainKeyEvent({ key: ' ', code: 'Space' }, 'ctrl+space')).toBe(true);
      expect(isMainKeyEvent({ key: 'Dead', code: 'KeyB' }, 'ctrl+b')).toBe(true);
      expect(isMainKeyEvent({ key: 'Control', code: 'ControlLeft' }, 'ctrl+a')).toBe(false);
    });
  });
});
