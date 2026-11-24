// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Onboarding state store
 * Tracks first-run onboarding progress, persisted to localStorage
 */

import { writable, get } from 'svelte/store';

interface OnboardingState {
  completed: boolean;
  currentStep: number;
  skipped: boolean;
}

const STORAGE_KEY = 'echophrase_onboarding';

/**
 * All FTUE-related localStorage keys - cleared together on reset.
 * Keep in sync with: DebugPanel.svelte "Welcome" button and any component
 * that persists FTUE state (e.g. OutputToastsContainer's history peek).
 */
const FTUE_KEYS = [STORAGE_KEY, 'echophrase_history_peek_shown'];

/**
 * Keys that used to be FTUE state but no longer exist. Removed once on init so
 * stale installs don't keep them around forever.
 * - echophrase_has_transcribed: the empty-state hint is per-session now (in-memory).
 */
const LEGACY_FTUE_KEYS = ['echophrase_has_transcribed'];

function purgeLegacyKeys() {
  if (typeof window === 'undefined') return;
  for (const key of LEGACY_FTUE_KEYS) {
    try {
      localStorage.removeItem(key);
    } catch {
      // localStorage may be unavailable; legacy keys are harmless if left behind
    }
  }
}

function loadState(): OnboardingState {
  if (typeof window === 'undefined') {
    return { completed: false, currentStep: 0, skipped: false };
  }
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored) return JSON.parse(stored);
  } catch {
    // Corrupted data - reset
  }
  return { completed: false, currentStep: 0, skipped: false };
}

function saveState(state: OnboardingState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // Storage full or unavailable
  }
}

function createOnboardingStore() {
  purgeLegacyKeys();
  const { subscribe, set, update } = writable<OnboardingState>(loadState());

  return {
    subscribe,

    /** Advance to the next onboarding step */
    nextStep() {
      update(state => {
        const next = { ...state, currentStep: state.currentStep + 1 };
        saveState(next);
        return next;
      });
    },

    /** Go back to the previous step */
    prevStep() {
      update(state => {
        const prev = { ...state, currentStep: Math.max(0, state.currentStep - 1) };
        saveState(prev);
        return prev;
      });
    },

    /** Mark onboarding as completed */
    complete() {
      const completed = { completed: true, currentStep: 0, skipped: false };
      set(completed);
      saveState(completed);
    },

    /** Skip onboarding (marks as completed) */
    skip() {
      const skipped = { completed: true, currentStep: 0, skipped: true };
      set(skipped);
      saveState(skipped);
    },

    /** Reset all FTUE state (onboarding, hints, etc.) */
    reset() {
      const fresh = { completed: false, currentStep: 0, skipped: false };
      set(fresh);
      saveState(fresh);
      // Clear all FTUE-related keys
      for (const key of FTUE_KEYS) {
        try {
          localStorage.removeItem(key);
        } catch {
          // localStorage may be unavailable; nothing to clear
        }
      }
      // Notify any mounted components that FTUE was reset
      window.dispatchEvent(new CustomEvent('ftue-reset'));
    },

    /** Check if onboarding should show */
    shouldShow(): boolean {
      return !get({ subscribe }).completed;
    },
  };
}

export const onboardingStore = createOnboardingStore();
