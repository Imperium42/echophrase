// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  beginRun,
  publishRun,
  restoreLatestRun,
  pushRun,
  lastRun,
  lastRuns,
  HISTORY_LIMIT,
  resolveHistoryLimit,
  setHistoryLimit,
  type LastRun,
  historyPeekRequested,
  requestHistoryPeek,
  HISTORY_PEEK_LINGER_MS,
} from './lastRun';

describe('requestHistoryPeek (Copy Last button hover shows the history chips)', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    requestHistoryPeek(false);
    vi.runAllTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows the chips immediately on hover', () => {
    requestHistoryPeek(true);
    expect(get(historyPeekRequested)).toBe(true);
  });

  it('lingers briefly after leaving so the mouse can reach the chips', () => {
    requestHistoryPeek(true);
    requestHistoryPeek(false);
    expect(get(historyPeekRequested)).toBe(true);
    vi.advanceTimersByTime(HISTORY_PEEK_LINGER_MS);
    expect(get(historyPeekRequested)).toBe(false);
  });

  it('re-hovering during the linger cancels the hide', () => {
    requestHistoryPeek(true);
    requestHistoryPeek(false);
    requestHistoryPeek(true);
    vi.advanceTimersByTime(HISTORY_PEEK_LINGER_MS * 2);
    expect(get(historyPeekRequested)).toBe(true);
  });

  it('a new recording drops a stuck peek (the button can vanish mid-hover)', () => {
    requestHistoryPeek(true);
    beginRun();
    expect(get(historyPeekRequested)).toBe(false);
  });
});

function makeRun(at: number): LastRun {
  return { raw: `run ${at}`, transformed: null, actions: [], speed: null, at };
}

describe('pushRun', () => {
  it('puts the newest run first', () => {
    const runs = pushRun([makeRun(1)], makeRun(2));
    expect(runs.map(r => r.at)).toEqual([2, 1]);
  });

  it('caps the history at the limit', () => {
    let runs: LastRun[] = [];
    for (let i = 1; i <= 12; i++) runs = pushRun(runs, makeRun(i));
    expect(runs).toHaveLength(HISTORY_LIMIT);
    expect(runs[0].at).toBe(12);
    expect(runs[HISTORY_LIMIT - 1].at).toBe(3);
  });

  it('does not mutate its input', () => {
    const input = Object.freeze([makeRun(1)]);
    const out = pushRun(input, makeRun(2));
    expect(input).toHaveLength(1);
    expect(out).not.toBe(input);
  });
});

describe('lastRuns history', () => {
  it('keeps the last 10 published runs, latest first', () => {
    for (let i = 1; i <= 12; i++) publishRun(makeRun(100 + i));
    const runs = get(lastRuns);
    expect(runs).toHaveLength(HISTORY_LIMIT);
    expect(runs[0].at).toBe(112);
    expect(get(lastRun)?.at).toBe(112);
  });

  it('a run published with remember: false (the demo) stays out of the history', () => {
    publishRun(makeRun(300));
    publishRun(makeRun(301), { remember: false });
    expect(get(lastRun)?.at).toBe(301);
    expect(get(lastRuns)[0].at).toBe(300);
    restoreLatestRun();
    expect(get(lastRun)?.at).toBe(300);
  });

  it('beginRun hides the latest run but keeps the history', () => {
    publishRun(makeRun(200));
    beginRun();
    expect(get(lastRun)).toBeNull();
    expect(get(lastRuns)[0].at).toBe(200);
  });
});

describe('resolveHistoryLimit (Remember last N transcripts setting)', () => {
  it('is 0 when history is turned off', () => {
    expect(resolveHistoryLimit(false, 5)).toBe(0);
  });

  it('defaults to the max when the count is missing (old settings file)', () => {
    expect(resolveHistoryLimit(undefined, undefined)).toBe(HISTORY_LIMIT);
    expect(resolveHistoryLimit(true, undefined)).toBe(HISTORY_LIMIT);
  });

  it('clamps the count to 1-10', () => {
    expect(resolveHistoryLimit(true, 0)).toBe(1);
    expect(resolveHistoryLimit(true, 99)).toBe(HISTORY_LIMIT);
    expect(resolveHistoryLimit(true, 4)).toBe(4);
  });
});

describe('setHistoryLimit', () => {
  afterEach(() => setHistoryLimit(HISTORY_LIMIT));

  it('lowering the limit drops the oldest runs from memory immediately', () => {
    for (let i = 1; i <= 6; i++) publishRun(makeRun(400 + i));
    setHistoryLimit(3);
    expect(get(lastRuns).map(r => r.at)).toEqual([406, 405, 404]);
  });

  it('new runs respect the lowered limit', () => {
    setHistoryLimit(2);
    for (let i = 1; i <= 5; i++) publishRun(makeRun(500 + i));
    expect(get(lastRuns).map(r => r.at)).toEqual([505, 504]);
  });

  it('turning history off forgets every run but still shows the current one', () => {
    publishRun(makeRun(600));
    setHistoryLimit(0);
    expect(get(lastRuns)).toEqual([]);
    publishRun(makeRun(601));
    expect(get(lastRuns)).toEqual([]);
    expect(get(lastRun)?.at).toBe(601);
  });
});
