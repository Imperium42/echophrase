// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable } from 'svelte/store';

/**
 * State for the scripted marketing demo (DebugPanel > Demo).
 *
 * The demo never touches the real recorder or the Rust backend, so these
 * stores are OR-ed into the real UI state by the consumers:
 * - `+page.svelte` flips the RecordButton / header timer with the first two.
 * - `OutputToastsContainer` force-shows the speed chip with `demoChipsPinned`.
 */

/**
 * True for the whole demo (countdown through the post-result grace period).
 * While set: top-left banners hidden, model chip mocked to
 * Parakeet 0.6B on GPU, and the empty-state hint shows the default hotkey.
 */
export const demoSession = writable<boolean>(false);

/** Whether the demo options modal is open. */
export const demoModalOpen = writable<boolean>(false);

/** Countdown number to display (10..3); null while hidden (incl. the silent 2..1). */
export const demoCountdown = writable<number | null>(null);

/**
 * True from the moment the countdown goes silent until the demo ends: hides the dev-only chrome
 * (top-left Debug button, bottom-right API endpoint toggle) so it never reaches the recording.
 */
export const demoChromeHidden = writable<boolean>(false);

/** Model shown in the model chip during the demo. */
export const DEMO_MODEL_ID = 'parakeet-tdt-0.6b-v3';
/** Execution provider shown in the model chip during the demo (DirectML = "GPU"). */
export const DEMO_PROVIDER = 'direct_ml';
/** Hotkey the hint advertises during the demo, regardless of the user's binding. */
export const DEMO_HOTKEY = 'ctrl+shift+space';

/** True while the demo is pretending to record. */
export const demoRecording = writable<boolean>(false);

/** Fake recording timer (whole seconds) shown while `demoRecording` is true. */
export const demoRecordingTime = writable<number>(0);

/** True from the fake Record click until the demo ends; hides the empty-state hint so it can't reappear under the bubble. */
export const demoActive = writable<boolean>(false);

/** Force the output chips (incl. compute time) visible without hover. */
export const demoChipsPinned = writable<boolean>(false);
