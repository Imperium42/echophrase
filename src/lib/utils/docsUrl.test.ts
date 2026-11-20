// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it, vi } from 'vitest';

let mockCurrentLocale: 'en' | 'zh-TW' | 'zh-CN' = 'en';

vi.mock('$lib/stores/locale.svelte', () => ({
  getCurrentLocale: () => mockCurrentLocale,
}));

import { docsUrl } from './docsUrl';

describe('docsUrl', () => {
  it('returns the root docs URL for the root path in English', () => {
    expect(docsUrl('')).toBe('https://docs.echophrase.com');
  });

  it('returns a nested path unprefixed in English', () => {
    expect(docsUrl('/features/custom-models')).toBe(
      'https://docs.echophrase.com/features/custom-models'
    );
  });

  it('preserves a hash fragment', () => {
    expect(docsUrl('/troubleshooting/permissions#mic')).toBe(
      'https://docs.echophrase.com/troubleshooting/permissions#mic'
    );
  });

  it('uses the explicit locale argument over the current locale', () => {
    expect(docsUrl('/mcp', 'zh-TW')).toBe('https://docs.echophrase.com/zh-Hant/mcp');
  });

  it('maps zh-CN to the /zh-CN docs prefix', () => {
    expect(docsUrl('/mcp', 'zh-CN')).toBe('https://docs.echophrase.com/zh-CN/mcp');
  });

  it('en passthrough when explicit locale is en', () => {
    expect(docsUrl('/mcp', 'en')).toBe('https://docs.echophrase.com/mcp');
  });

  it('falls back to the current locale when locale is omitted', () => {
    mockCurrentLocale = 'zh-TW';
    expect(docsUrl('/cli')).toBe('https://docs.echophrase.com/zh-Hant/cli');
    mockCurrentLocale = 'en';
  });
});
