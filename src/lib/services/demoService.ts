// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { get } from 'svelte/store';
import { createDemoCursor, type DemoCursor } from '$lib/services/demoCursor';
import {
  demoRecording,
  demoRecordingTime,
  demoChipsPinned,
  demoActive,
  demoSession,
  demoCountdown,
  demoChromeHidden,
  demoModalOpen,
} from '$lib/stores/demo';
import { publishRun, lastRun } from '$lib/stores/lastRun';

/**
 * Scripted, self-running marketing demo: fake cursor clicks Record, then Stop,
 * then a canned transcription appears. Never touches Tauri, audio, or the
 * backend - it only drives UI state and existing display seams.
 */

/** User-tunable timings (seconds) and content, set in the Demo options modal. */
export interface DemoOptions {
  /** Visible countdown after the reload (lets the user get out of frame). */
  startDelaySec: number;
  /** Extra pause after the countdown before the fake cursor starts moving. */
  moveDelaySec: number;
  /** How long the fake recording lasts before the cursor clicks Stop. */
  recordSec: number;
  /** Mocked GPU/model state lingers this long after the demo finishes, then resets. */
  resetGraceSec: number;
  /** Canned transcription text shown as the result. */
  transcriptionText: string;
}

/** Canned transcription (the real slogan from the marketing site). */
export const DEMO_TEXT = 'Save your fingers and type faster - free and unlimited.';

/** Defaults shown in the options modal. */
export const DEFAULT_DEMO_OPTIONS: DemoOptions = {
  startDelaySec: 10,
  moveDelaySec: 3,
  recordSec: 2,
  resetGraceSec: 10,
  transcriptionText: DEMO_TEXT,
};

const MAX_OPTION_SEC = 600;
const MAX_TEXT_LEN = 500;
/** localStorage key remembering the last-used options as the next defaults. */
const OPTIONS_KEY = 'echophrase-demo-options';

type DemoTimingKey = 'startDelaySec' | 'moveDelaySec' | 'recordSec' | 'resetGraceSec';

/** Coerce arbitrary input to valid options (whole seconds, 0..600), falling back to defaults. */
export function sanitizeDemoOptions(raw: unknown): DemoOptions {
  const src: Partial<Record<keyof DemoOptions, unknown>> =
    typeof raw === 'object' && raw !== null ? raw : {};
  const pick = (k: DemoTimingKey): number => {
    const n: number = Math.round(Number(src[k]));
    return Number.isFinite(n) && n >= 0 && n <= MAX_OPTION_SEC ? n : DEFAULT_DEMO_OPTIONS[k];
  };
  const text: unknown = src.transcriptionText;
  const transcriptionText: string =
    typeof text === 'string' && text.trim().length > 0
      ? text.slice(0, MAX_TEXT_LEN)
      : DEFAULT_DEMO_OPTIONS.transcriptionText;
  return {
    startDelaySec: pick('startDelaySec'),
    moveDelaySec: pick('moveDelaySec'),
    recordSec: pick('recordSec'),
    resetGraceSec: pick('resetGraceSec'),
    transcriptionText,
  };
}

/** Last-used options (or defaults). */
export function loadDemoOptions(): DemoOptions {
  try {
    const stored: string | null = localStorage.getItem(OPTIONS_KEY);
    return sanitizeDemoOptions(stored ? JSON.parse(stored) : null);
  } catch {
    return { ...DEFAULT_DEMO_OPTIONS };
  }
}
/** How long the fake recording lasts before the cursor clicks Stop (default option value, in ms). */
export const DEMO_RECORD_MS = DEFAULT_DEMO_OPTIONS.recordSec * 1000;
/** How long the compute-time chip stays pinned after the result lands. */
export const DEMO_CHIPS_PINNED_MS = 8_000;
/** Countdown numbers below this stay hidden so they never land in the recording. */
const COUNTDOWN_MIN_VISIBLE = 3;
/** sessionStorage flag: the Demo click reloads the window, the fresh page then runs the script. */
const PENDING_KEY = 'echophrase-demo-pending';
/** Fake compute time in seconds (0.1 ms). */
export const DEMO_COMPUTE_SECONDS = 0.0001;

const RECORD_BUTTON_SELECTOR = '.record-btn';
/** Where on the record button the cursor lands: lower right of center, like a real hand resting on it. */
const RECORD_BUTTON_AIM = { x: 0.28, y: 0.26 };

let controller: AbortController | null = null;
let cursor: DemoCursor | null = null;
let timerInterval: ReturnType<typeof setInterval> | null = null;

/** Whether a demo is currently running. */
export function isDemoRunning(): boolean {
  return controller !== null;
}

/** Abortable sleep; rejects with an AbortError-like when cancelled. */
function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise<void>((resolve, reject) => {
    if (signal.aborted) return reject(new Error('demo cancelled'));
    const id = setTimeout(() => {
      signal.removeEventListener('abort', onAbort);
      resolve();
    }, ms);
    const onAbort = (): void => {
      clearTimeout(id);
      reject(new Error('demo cancelled'));
    };
    signal.addEventListener('abort', onAbort, { once: true });
  });
}

/** Reset every piece of demo state and DOM. Safe to call repeatedly. */
function cleanup(): void {
  if (timerInterval) {
    clearInterval(timerInterval);
    timerInterval = null;
  }
  cursor?.dispose();
  cursor = null;
  demoRecording.set(false);
  demoRecordingTime.set(0);
  demoChipsPinned.set(false);
  demoActive.set(false);
  demoCountdown.set(null);
  demoChromeHidden.set(false);
  demoSession.set(false);
  controller = null;
}

/** Cancel a running demo and tear down its UI. No-op when idle. */
export function cancelDemo(): void {
  controller?.abort();
  cleanup();
}

/** Fire the canned transcript through the real preview bubble (same event the recorder uses). */
function publishMockResult(text: string): void {
  window.dispatchEvent(
    new CustomEvent('transcription-preview', { detail: { text, is_final: true } })
  );
  publishRun({
    raw: text,
    transformed: null,
    output: text,
    actions: ['copy_to_clipboard'],
    speed: {
      transcriptionSeconds: DEMO_COMPUTE_SECONDS,
      transformSeconds: null,
      totalSeconds: DEMO_COMPUTE_SECONDS,
    },
    at: Date.now(),
  });
  demoChipsPinned.set(true);
}

/** Demo button handler: opens the options modal, or cancels a demo that is already running. */
export function requestDemo(): void {
  if (isDemoRunning()) cancelDemo();
  else demoModalOpen.set(true);
}

/**
 * Modal "Start": remember the options, hard-reload the window (like Ctrl+Shift+R) so
 * the recording starts from a pristine UI, then resume via {@link resumeDemoIfPending}.
 */
export function startDemoWithReload(options: DemoOptions): void {
  const clean: DemoOptions = sanitizeDemoOptions(options);
  demoModalOpen.set(false);
  try {
    localStorage.setItem(OPTIONS_KEY, JSON.stringify(clean));
    sessionStorage.setItem(PENDING_KEY, JSON.stringify(clean));
  } catch {
    // No storage: run in place without the reload.
    void runDemo(clean);
    return;
  }
  window.location.reload();
}

/** Call once on page mount (dev only): starts the demo if a reload was requested. */
export function resumeDemoIfPending(): void {
  let options: DemoOptions;
  try {
    const pending: string | null = sessionStorage.getItem(PENDING_KEY);
    if (pending === null) return;
    sessionStorage.removeItem(PENDING_KEY);
    options = sanitizeDemoOptions(JSON.parse(pending));
  } catch {
    return;
  }
  void runDemo(options);
}

/** Tick the visible countdown down from `seconds`, hiding the last numbers. */
async function countdown(seconds: number, signal: AbortSignal): Promise<void> {
  for (let n = seconds; n > 0; n--) {
    demoCountdown.set(n >= COUNTDOWN_MIN_VISIBLE ? n : null);
    if (n < COUNTDOWN_MIN_VISIBLE) demoChromeHidden.set(true);
    await sleep(1000, signal);
  }
  demoCountdown.set(null);
  demoChromeHidden.set(true);
}

/**
 * Run the demo script. Resolves when finished or cancelled. A second call
 * while one is running is ignored.
 */
export async function runDemo(options: DemoOptions = DEFAULT_DEMO_OPTIONS): Promise<void> {
  if (controller) return;
  const ctrl: AbortController = new AbortController();
  controller = ctrl;
  const { signal } = ctrl;

  demoSession.set(true);

  try {
    await countdown(options.startDelaySec, signal);
    await sleep(options.moveDelaySec * 1000, signal);

    const button: Element | null = document.querySelector(RECORD_BUTTON_SELECTOR);
    if (!button) {
      console.warn('[demo] .record-btn not found; aborting');
      return;
    }
    const c: DemoCursor = createDemoCursor();
    cursor = c;

    await c.moveTo(button, RECORD_BUTTON_AIM);
    signal.throwIfAborted();
    await c.click();
    signal.throwIfAborted();
    demoRecordingTime.set(0);
    demoActive.set(true);
    demoRecording.set(true);
    timerInterval = setInterval(() => demoRecordingTime.update(t => t + 1), 1000);

    await sleep(options.recordSec * 1000, signal);

    // The record button becomes the stop button in place, but re-query in case it re-rendered.
    await c.moveTo(document.querySelector(RECORD_BUTTON_SELECTOR) ?? button, RECORD_BUTTON_AIM);
    signal.throwIfAborted();
    await c.click();
    signal.throwIfAborted();
    if (timerInterval) {
      clearInterval(timerInterval);
      timerInterval = null;
    }
    demoRecording.set(false);
    publishMockResult(options.transcriptionText);

    await c.fadeOutAndRemove();
    cursor = null;

    await sleep(DEMO_CHIPS_PINNED_MS, signal);
    if (get(demoChipsPinned)) lastRun.clear();
    demoChipsPinned.set(false);

    // Keep the mocked GPU/model state a bit longer, then `finally` resets it.
    await sleep(options.resetGraceSec * 1000, signal);
  } catch (e) {
    if (!signal.aborted) console.error('[demo] failed', e);
  } finally {
    // Only tear down if we are still the active run (cancelDemo may have already reset).
    if (controller === ctrl) cleanup();
  }
}
