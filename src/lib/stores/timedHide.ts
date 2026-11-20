// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Timed Hide Store Factory
 *
 * Builds a boolean store that hides something for a fixed window and then
 * reveals it again, surviving a page reload via localStorage.
 *
 * Used for "get this off screen while I record a promo video / stream" toggles.
 * Extracted so the debug panel and the top-left banners share one implementation
 * instead of two copies of the same timer + localStorage dance.
 */

import { writable } from 'svelte/store';

/** Default hide window. Long enough to record a take, short enough to self-heal. */
export const HIDE_WINDOW_MS = 5 * 60 * 1000;

// Browser check without SvelteKit dependency (works in Tauri)
const isBrowser = typeof window !== 'undefined';

export interface TimedHideOptions {
  /** localStorage key holding the epoch-ms timestamp to reveal at. */
  key: string;
  /** Human label used in console breadcrumbs (e.g. "Debug sections"). */
  label: string;
  /**
   * When false the store is permanently `false` and every method is a no-op.
   * Pass `import.meta.env.DEV` so a stale localStorage key can never hide UI in
   * a production build.
   */
  enabled?: boolean;
}

export interface TimedHideStore {
  subscribe: ReturnType<typeof writable<boolean>>['subscribe'];
  /** Hide for the standard window (5 minutes). */
  hideFor5Minutes(): void;
  /** Reveal immediately and clear the persisted deadline. */
  show(): void;
  /** Reveal if the deadline has already passed (cheap external poll). */
  checkExpiry(): void;
}

/** Create a persisted, self-expiring visibility store. */
export function createTimedHideStore({
  key,
  label,
  enabled = true,
}: TimedHideOptions): TimedHideStore {
  const active = isBrowser && enabled;

  // Check initial state from localStorage
  let initialHidden = false;
  if (active) {
    const hideUntil = localStorage.getItem(key);
    if (hideUntil && Date.now() < parseInt(hideUntil, 10)) {
      initialHidden = true;
    } else {
      localStorage.removeItem(key);
    }
  }

  const { subscribe, set } = writable(initialHidden);

  let revealTimer: ReturnType<typeof setTimeout> | null = null;

  /** Schedule an in-session timer that reveals when the hide window elapses. */
  function scheduleReveal(hideUntil: number) {
    if (!active) return;
    if (revealTimer) clearTimeout(revealTimer);
    const delay = Math.max(0, hideUntil - Date.now());
    revealTimer = setTimeout(() => {
      revealTimer = null;
      localStorage.removeItem(key);
      set(false);
      console.log(`👀 ${label} visible (hide window elapsed)`);
    }, delay);
  }

  // Resume the reveal timer if we loaded into a still-hidden state.
  if (initialHidden) {
    scheduleReveal(parseInt(localStorage.getItem(key) ?? '0', 10));
  }

  return {
    subscribe,

    hideFor5Minutes() {
      if (!active) return;
      const hideUntil = Date.now() + HIDE_WINDOW_MS;
      localStorage.setItem(key, hideUntil.toString());
      set(true);
      scheduleReveal(hideUntil);
      console.log(`🙈 ${label} hidden for 5 minutes`);
    },

    show() {
      if (!active) return;
      if (revealTimer) {
        clearTimeout(revealTimer);
        revealTimer = null;
      }
      localStorage.removeItem(key);
      set(false);
      console.log(`👀 ${label} visible`);
    },

    checkExpiry() {
      if (!active) return;
      const hideUntil = localStorage.getItem(key);
      if (hideUntil && Date.now() >= parseInt(hideUntil, 10)) {
        localStorage.removeItem(key);
        set(false);
      }
    },
  };
}
