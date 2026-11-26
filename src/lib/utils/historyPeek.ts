// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * The history hint (the left-side chip stack fading in once, with a "hover here"
 * caption) plays once per app session. sessionStorage survives an F5 reload but
 * is wiped when the app quits, which is exactly "once per session".
 *
 * It used to be once ever (localStorage); that key is purged as legacy FTUE state
 * in `stores/onboarding.ts`.
 */
const SEEN_KEY = 'echophrase_history_peek_shown';

export function hasSeenHistoryPeek(): boolean {
  try {
    return sessionStorage.getItem(SEEN_KEY) === 'true';
  } catch {
    return false; // storage unavailable: show the hint again, harmless
  }
}

export function markHistoryPeekSeen(): void {
  try {
    sessionStorage.setItem(SEEN_KEY, 'true');
  } catch {
    // storage unavailable: the hint may show again after a reload
  }
}

export function resetHistoryPeekSeen(): void {
  try {
    sessionStorage.removeItem(SEEN_KEY);
  } catch {
    // storage unavailable: nothing to reset
  }
}
