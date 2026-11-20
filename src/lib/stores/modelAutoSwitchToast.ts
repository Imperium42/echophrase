// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable } from 'svelte/store';

const AUTO_SWITCH_TOAST_DURATION_MS = 8000;

export interface ModelAutoSwitchInfo {
  /** Model id that was selected but unavailable */
  from: string;
  /** Model id that was auto-selected instead */
  to: string;
}

const store = writable<ModelAutoSwitchInfo | null>(null);

export const modelAutoSwitchToast = {
  subscribe: store.subscribe,
  dismiss: () => store.set(null),
};

/**
 * Show a notice that the selected model was unavailable and the app
 * auto-switched (and persisted) a different one. Called from
 * modelStore.initialize() so the change is never silent.
 */
export function showModelAutoSwitchToast(info: ModelAutoSwitchInfo): void {
  store.set(info);
  setTimeout(() => store.set(null), AUTO_SWITCH_TOAST_DURATION_MS);
}
