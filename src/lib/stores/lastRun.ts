// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { get, writable } from 'svelte/store';

export type LastRunAction = 'copy_to_clipboard' | 'paste_at_cursor' | 'save_as_file';

export interface LastRunSpeed {
  transcriptionSeconds: number;
  transformSeconds: number | null;
  totalSeconds: number;
}

export interface LastRun {
  /** Raw transcript text before any AI transform. */
  raw: string;
  /** Transformed text if AI transform ran; else null. */
  transformed: string | null;
  /** The final formatted output string actually delivered to the user
   *  (post-filler-removal, post-digit-conversion, post-format). Undefined if no
   *  output action fired. */
  output?: string;
  /** Actions that were executed for this run, in the order they occurred. */
  actions: LastRunAction[];
  /** Speed stats if show_transcription_speed is enabled; null otherwise. */
  speed: LastRunSpeed | null;
  /** When this run completed. */
  at: number;
}

/** Most completed runs the in-memory history can keep (never written to disk).
 *  Keep in step with MAX_HISTORY_LIMIT in src-tauri/src/settings/mod.rs. */
export const HISTORY_LIMIT = 10;
/** Fewest runs a user can pick while history is on. */
export const MIN_HISTORY_LIMIT = 1;

/**
 * Effective history size from the user's settings: 0 when history is off,
 * else the chosen count clamped to 1-HISTORY_LIMIT (missing = HISTORY_LIMIT).
 */
export function resolveHistoryLimit(
  enabled: boolean | undefined,
  limit: number | undefined
): number {
  if (enabled === false) return 0;
  const n = Math.round(limit ?? HISTORY_LIMIT);
  if (!Number.isFinite(n)) return HISTORY_LIMIT;
  return Math.min(HISTORY_LIMIT, Math.max(MIN_HISTORY_LIMIT, n));
}

/** Prepend `run` and cap at `limit`. Returns a new array; never mutates `runs`. */
export function pushRun(
  runs: readonly LastRun[],
  run: LastRun,
  limit: number = HISTORY_LIMIT
): LastRun[] {
  return [run, ...runs].slice(0, limit);
}

const store = writable<LastRun | null>(null);
const runsStore = writable<LastRun[]>([]);
let historyLimit = HISTORY_LIMIT;

/**
 * Apply the user's history settings. Lowering the count (or turning history
 * off) drops the extra runs from memory right away, not on the next recording.
 */
export function setHistoryLimit(limit: number): void {
  historyLimit = Math.max(0, Math.min(HISTORY_LIMIT, Math.round(limit)));
  runsStore.update(runs => (runs.length > historyLimit ? runs.slice(0, historyLimit) : runs));
}

/** The last `historyLimit` completed runs, newest first. In memory only. */
export const lastRuns = { subscribe: runsStore.subscribe };

/** Subscribe to the most recent completed run (null if none yet). */
export const lastRun = {
  subscribe: store.subscribe,
  clear: (): void => store.set(null),
};

/**
 * Begin a new run - called at recording start. Clears the store so the old
 * run's chips disappear while the new run is in progress. The history in
 * `lastRuns` is kept.
 */
export function beginRun(): void {
  store.set(null);
  // The Copy Last button unmounts when recording starts, so its mouseleave
  // may never fire; drop any peek it requested.
  if (peekLingerTimer) {
    clearTimeout(peekLingerTimer);
    peekLingerTimer = null;
  }
  peekRequested.set(false);
}

/** Put the newest remembered run back (null if none), eg after the demo's fake run. */
export function restoreLatestRun(): void {
  store.set(get(runsStore)[0] ?? null);
}

export interface PublishRunOptions {
  /** Add the run to the `lastRuns` history (default true). The scripted demo
   *  passes false so its fake run never shows up as a real recording. */
  remember?: boolean;
}

/** Finalize and publish a completed run in one atomic update. */
export function publishRun(run: LastRun, { remember = true }: PublishRunOptions = {}): void {
  store.set(run);
  if (remember) runsStore.update(runs => pushRun(runs, run, historyLimit));
}

/**
 * How long the history chips stay up after the Copy Last button loses hover,
 * so the mouse can travel from the bottom-left button up to the chips.
 */
export const HISTORY_PEEK_LINGER_MS = 600;

const peekRequested = writable<boolean>(false);
let peekLingerTimer: ReturnType<typeof setTimeout> | null = null;

/** True while another control (the bottom-left Copy Last button) asks to show the history chips. */
export const historyPeekRequested = { subscribe: peekRequested.subscribe };

/** Show (true) or, after a short linger, hide (false) the history chips from outside the chip area. */
export function requestHistoryPeek(show: boolean): void {
  if (peekLingerTimer) {
    clearTimeout(peekLingerTimer);
    peekLingerTimer = null;
  }
  if (show) {
    peekRequested.set(true);
    return;
  }
  peekLingerTimer = setTimeout(() => {
    peekLingerTimer = null;
    peekRequested.set(false);
  }, HISTORY_PEEK_LINGER_MS);
}
