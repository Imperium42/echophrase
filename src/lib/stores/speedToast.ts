// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Speed toast store.
 *
 * Shows a brief timing summary after each transcription completes.
 */

import { writable } from 'svelte/store';

const SPEED_TOAST_DURATION_MS = 4000;

export interface SpeedToast {
  id: string;
  transcriptionSeconds: number;
  transformSeconds: number | null;
  totalSeconds: number;
}

const speedToastStore = writable<SpeedToast | null>(null);

/** Subscribe to the current speed toast (null when hidden). */
export const speedToast = {
  subscribe: speedToastStore.subscribe,
};

/** Show the speed toast for SPEED_TOAST_DURATION_MS then auto-dismiss. */
export function showSpeedToast(
  transcriptionSeconds: number,
  transformSeconds: number | null = null
): void {
  const totalSeconds = transcriptionSeconds + (transformSeconds ?? 0);
  speedToastStore.set({
    id: `speed-${Date.now()}`,
    transcriptionSeconds,
    transformSeconds,
    totalSeconds,
  });
  setTimeout(() => speedToastStore.set(null), SPEED_TOAST_DURATION_MS);
}
