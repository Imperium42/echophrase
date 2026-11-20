// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import { formatLongDate } from './formatters';

describe('formatLongDate', () => {
  const iso = '2027-09-03T12:00:00Z';

  it('formats in English for en', () => {
    expect(formatLongDate(iso, 'en')).toBe('September 3, 2027');
  });

  it('formats in Traditional Chinese for zh-TW regardless of WebView language', () => {
    expect(formatLongDate(iso, 'zh-TW')).toBe('2027年9月3日');
  });

  it('formats in Simplified Chinese for zh-CN', () => {
    expect(formatLongDate(iso, 'zh-CN')).toBe('2027年9月3日');
  });

  it('returns the raw input for an unparseable date', () => {
    expect(formatLongDate('nope', 'en')).toBe('nope');
  });
});
