// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach } from 'vitest';
import { hasSeenHistoryPeek, markHistoryPeekSeen, resetHistoryPeekSeen } from './historyPeek';

describe('history peek hint: once per app session', () => {
  beforeEach(() => {
    sessionStorage.clear();
    localStorage.clear();
  });

  it('has not been seen in a fresh session', () => {
    expect(hasSeenHistoryPeek()).toBe(false);
  });

  it('is seen after marking, for the rest of the session', () => {
    markHistoryPeekSeen();
    expect(hasSeenHistoryPeek()).toBe(true);
  });

  it('does not persist across app sessions (sessionStorage, not localStorage)', () => {
    markHistoryPeekSeen();
    sessionStorage.clear(); // app restart
    expect(hasSeenHistoryPeek()).toBe(false);
  });

  it('ignores the old forever-flag in localStorage', () => {
    localStorage.setItem('echophrase_history_peek_shown', 'true');
    expect(hasSeenHistoryPeek()).toBe(false);
  });

  it('can be reset (debug panel / FTUE reset)', () => {
    markHistoryPeekSeen();
    resetHistoryPeekSeen();
    expect(hasSeenHistoryPeek()).toBe(false);
  });
});
