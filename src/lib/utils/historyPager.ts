// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Pure helpers for the left-side history pager. Index 0 is the latest run;
 * a positive delta steps toward older runs.
 */

/** Move `delta` steps through a history of `length` runs, clamped to the ends. */
export function stepHistory(index: number, delta: number, length: number): number {
  if (length <= 1) return 0;
  return Math.min(Math.max(index + delta, 0), length - 1);
}

/** 1-based position label, eg "1/10". */
export function historyLabel(index: number, length: number): string {
  return `${index + 1}/${length}`;
}

export type PagerArrow = 'older' | 'newer';
export type PagerCaption = PagerArrow | 'latest' | 'oldest';

/**
 * Caption beside the pager: the hovered arrow's direction when it can move,
 * else "latest"/"oldest" at either end, else nothing.
 */
export function pagerCaption(
  index: number,
  length: number,
  hovered: PagerArrow | null
): PagerCaption | null {
  const atLatest = index <= 0;
  const atOldest = index >= length - 1;
  if (hovered === 'newer' && !atLatest) return 'newer';
  if (hovered === 'older' && !atOldest) return 'older';
  if (atLatest) return 'latest';
  if (atOldest) return 'oldest';
  return null;
}
