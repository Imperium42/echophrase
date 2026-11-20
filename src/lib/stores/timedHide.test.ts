// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { createTimedHideStore, HIDE_WINDOW_MS } from './timedHide';

const KEY = 'test-hide-until';

describe('createTimedHideStore', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  function make(enabled = true) {
    return createTimedHideStore({ key: KEY, label: 'Test', enabled });
  }

  it('starts visible when nothing is persisted', () => {
    expect(get(make())).toBe(false);
  });

  it('hides for the 5 minute window and persists the deadline', () => {
    const store = make();
    store.hideFor5Minutes();

    expect(get(store)).toBe(true);
    const deadline = parseInt(localStorage.getItem(KEY) ?? '0', 10);
    expect(deadline).toBe(Date.now() + HIDE_WINDOW_MS);
  });

  it('reveals automatically once the window elapses', () => {
    const store = make();
    store.hideFor5Minutes();

    vi.advanceTimersByTime(HIDE_WINDOW_MS - 1);
    expect(get(store)).toBe(true);

    vi.advanceTimersByTime(1);
    expect(get(store)).toBe(false);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it('show() reveals early and clears the deadline', () => {
    const store = make();
    store.hideFor5Minutes();
    store.show();

    expect(get(store)).toBe(false);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it('does not re-hide after show() when the original window would have elapsed', () => {
    const store = make();
    store.hideFor5Minutes();
    store.show();

    vi.advanceTimersByTime(HIDE_WINDOW_MS * 2);
    expect(get(store)).toBe(false);
  });

  it('restores a still-active hide window from localStorage', () => {
    localStorage.setItem(KEY, (Date.now() + 60_000).toString());
    expect(get(make())).toBe(true);
  });

  it('ignores and clears an already-expired deadline', () => {
    localStorage.setItem(KEY, (Date.now() - 1).toString());

    expect(get(make())).toBe(false);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it('checkExpiry() reveals when the deadline has passed', () => {
    const store = make();
    store.hideFor5Minutes();

    // Jump past the deadline without letting the scheduled timer fire.
    vi.setSystemTime(Date.now() + HIDE_WINDOW_MS + 1);
    store.checkExpiry();

    expect(get(store)).toBe(false);
  });

  it('is inert when disabled - never hides and never writes localStorage', () => {
    const store = make(false);
    store.hideFor5Minutes();

    expect(get(store)).toBe(false);
    expect(localStorage.getItem(KEY)).toBeNull();
  });

  it('ignores a persisted deadline when disabled (stale key cannot hide prod UI)', () => {
    localStorage.setItem(KEY, (Date.now() + 60_000).toString());
    expect(get(make(false))).toBe(false);
  });

  it('keeps separate keys independent', () => {
    const a = createTimedHideStore({ key: 'hide-a', label: 'A' });
    const b = createTimedHideStore({ key: 'hide-b', label: 'B' });

    a.hideFor5Minutes();

    expect(get(a)).toBe(true);
    expect(get(b)).toBe(false);
  });
});
