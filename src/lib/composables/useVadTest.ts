// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Voice Activity Detection live-test composable.
 *
 * Owns the full lifecycle of the "Test" button in Settings > Audio Input:
 *   - subscribes to the `vad-level` Tauri event for live RMS/probability
 *   - converts RMS to a 0-100 dB-scaled meter level
 *   - tracks a peak-hold indicator with smooth decay
 *   - auto-stops after 30s so a forgotten test session doesn't burn CPU
 *
 * Extracted from AudioInputSection.svelte so the section component shrinks
 * back to layout + settings glue, and so the test transport is unit-
 * testable without mounting the dialog.
 */

import { writable, type Readable } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { mustInvoke, safeInvoke } from '$lib/ipc/invoke';
import { devError } from '$lib/utils/devLog';

/** Cap on how long a single test session runs before auto-stopping. */
const TEST_TIMEOUT_MS = 30_000;
/** Peak indicator decay step (per 50ms tick). */
const PEAK_FADE_PER_TICK = 0.05;
/** Peak level drop step once opacity has reached zero. */
const PEAK_DROP_PER_TICK = 2;

interface VadLevelEvent {
  probability: number;
  is_speech: boolean;
  peak_amplitude: number;
  rms_level: number;
}

export interface VadTestState {
  isRunning: Readable<boolean>;
  /** 0-100 dB-scaled meter level, derived from RMS. */
  level: Readable<number>;
  /** 0-100 peak-hold position. */
  peak: Readable<number>;
  /** 0-1 peak-marker opacity (fades to 0 then drops the peak). */
  peakOpacity: Readable<number>;
  /** Whether the VAD probability currently classifies as speech. */
  isSpeech: Readable<boolean>;
}

export interface VadTestActions {
  /** Toggle the test on/off. Provide the audio device id when starting. */
  toggle: (deviceId: string) => Promise<void>;
  /** Stop the test if running and release the listener / timer. */
  stop: () => Promise<void>;
  /** Clean up timers + listener without firing the IPC stop call. */
  cleanup: () => void;
}

export type VadTest = VadTestState & VadTestActions;

export function useVadTest(): VadTest {
  const isRunning = writable(false);
  const level = writable(0);
  const peak = writable(0);
  const peakOpacity = writable(0);
  const isSpeech = writable(false);

  let unlisten: UnlistenFn | null = null;
  let testTimeout: ReturnType<typeof setTimeout> | null = null;
  let peakDecayInterval: ReturnType<typeof setInterval> | null = null;

  function rmsToMeterLevel(rms: number): number {
    if (rms <= 0.0001) return 0;
    // Convert to dB: 20 * log10(rms). Typical speech RMS is 0.01-0.1 ≈ -40dB to -20dB.
    // Map so average speech (-35dB) shows ~50%, silence (-60dB) shows 0%.
    const db = 20 * Math.log10(rms);
    return Math.max(0, Math.min(100, Math.round((db + 60) * 2)));
  }

  function clearTimers() {
    if (testTimeout) {
      clearTimeout(testTimeout);
      testTimeout = null;
    }
    if (peakDecayInterval) {
      clearInterval(peakDecayInterval);
      peakDecayInterval = null;
    }
  }

  function detachListener() {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
  }

  function reset() {
    isRunning.set(false);
    level.set(0);
    peak.set(0);
    peakOpacity.set(0);
    isSpeech.set(false);
  }

  async function start(deviceId: string): Promise<void> {
    try {
      unlisten = await listen<VadLevelEvent>('vad-level', event => {
        const newLevel = rmsToMeterLevel(event.payload.rms_level);
        level.set(newLevel);

        // Peak hold: rise instantly, fall slowly.
        peak.update(current => {
          if (newLevel > current) {
            peakOpacity.set(1);
            return newLevel;
          }
          return current;
        });

        isSpeech.set(event.payload.is_speech);
      });

      await mustInvoke<void>('start_vad_test', { deviceId });
      isRunning.set(true);

      peakDecayInterval = setInterval(() => {
        let opacityNow = 0;
        peakOpacity.update(o => {
          opacityNow = Math.max(0, o - PEAK_FADE_PER_TICK);
          return opacityNow;
        });
        if (opacityNow === 0) {
          peak.update(p => Math.max(0, p - PEAK_DROP_PER_TICK));
        }
      }, 50);

      testTimeout = setTimeout(() => {
        void stop();
      }, TEST_TIMEOUT_MS);
    } catch (err) {
      devError('[VAD Test] Failed to start:', err);
      reset();
      detachListener();
    }
  }

  async function stop(): Promise<void> {
    clearTimers();
    detachListener();
    // Best-effort: a stop call after the backend already aborted should not crash the UI.
    await safeInvoke<void>('stop_vad_test');
    reset();
  }

  async function toggle(deviceId: string): Promise<void> {
    let running = false;
    isRunning.subscribe(v => (running = v))();
    if (running) {
      await stop();
    } else {
      await start(deviceId);
    }
  }

  function cleanup() {
    clearTimers();
    detachListener();
  }

  return {
    isRunning: { subscribe: isRunning.subscribe },
    level: { subscribe: level.subscribe },
    peak: { subscribe: peak.subscribe },
    peakOpacity: { subscribe: peakOpacity.subscribe },
    isSpeech: { subscribe: isSpeech.subscribe },
    toggle,
    stop,
    cleanup,
  };
}
