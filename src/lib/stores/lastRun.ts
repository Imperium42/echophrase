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

/** How many completed runs the in-memory history keeps (never written to disk). */
export const HISTORY_LIMIT = 10;

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

/** The last HISTORY_LIMIT completed runs, newest first. In memory only. */
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
  if (remember) runsStore.update(runs => pushRun(runs, run));
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
