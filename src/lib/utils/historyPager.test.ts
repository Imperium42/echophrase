// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { stepHistory, historyLabel, pagerCaption } from './historyPager';

describe('stepHistory (index 0 = latest run)', () => {
  it('steps toward older runs', () => {
    expect(stepHistory(0, 1, 10)).toBe(1);
  });

  it('steps toward newer runs', () => {
    expect(stepHistory(3, -1, 10)).toBe(2);
  });

  it('clamps at the latest run', () => {
    expect(stepHistory(0, -1, 10)).toBe(0);
  });

  it('clamps at the oldest run', () => {
    expect(stepHistory(9, 1, 10)).toBe(9);
  });

  it('stays at 0 for an empty or single-run history', () => {
    expect(stepHistory(0, 1, 0)).toBe(0);
    expect(stepHistory(0, 1, 1)).toBe(0);
  });

  it('pulls a stale index back in range when the history shrinks', () => {
    expect(stepHistory(7, 0, 3)).toBe(2);
  });
});

describe('historyLabel', () => {
  it('formats a 1-based position', () => {
    expect(historyLabel(0, 10)).toBe('1/10');
    expect(historyLabel(9, 10)).toBe('10/10');
  });
});

describe('pagerCaption', () => {
  it('shows Latest on the first page and Oldest on the last when no arrow is hovered', () => {
    expect(pagerCaption(0, 5, null)).toBe('latest');
    expect(pagerCaption(4, 5, null)).toBe('oldest');
    expect(pagerCaption(2, 5, null)).toBeNull();
  });

  it('a hovered arrow that can move wins', () => {
    expect(pagerCaption(0, 5, 'older')).toBe('older');
    expect(pagerCaption(4, 5, 'newer')).toBe('newer');
    expect(pagerCaption(2, 5, 'newer')).toBe('newer');
  });

  it('a hovered disabled arrow falls back to Latest/Oldest', () => {
    expect(pagerCaption(0, 5, 'newer')).toBe('latest');
    expect(pagerCaption(4, 5, 'older')).toBe('oldest');
  });
});
