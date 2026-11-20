// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable } from 'svelte/store';

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

const store = writable<LastRun | null>(null);

/** Subscribe to the most recent completed run (null if none yet). */
export const lastRun = {
  subscribe: store.subscribe,
  clear: (): void => store.set(null),
};

/**
 * Begin a new run — called at recording start. Clears the store so the old
 * run's chips disappear while the new run is in progress.
 *
 * TODO(future): maintain a `lastRuns: LastRun[]` history array (cap N) for a
 * multi-run recall view.
 */
export function beginRun(): void {
  store.set(null);
}

/** Finalize and publish a completed run in one atomic update. */
export function publishRun(run: LastRun): void {
  store.set(run);
}
