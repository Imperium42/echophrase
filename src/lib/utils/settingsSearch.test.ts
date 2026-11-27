// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import en from '$lib/locales/en.json';
import { buildSearchIndex, matchesSettingsSection, parseTerms } from './settingsSearch';

describe('parseTerms', () => {
  it('splits, trims, lowercases and drops empties', () => {
    expect(parseTerms(' GPU, Custom Model ,, vram ')).toEqual(['gpu', 'custom model', 'vram']);
    expect(parseTerms(undefined)).toEqual([]);
  });
});

describe('buildSearchIndex', () => {
  it('merges tables and dedupes terms', () => {
    const index = buildSearchIndex([{ gpu: 'gpu, vram' }, { gpu: 'GPU, 顯卡' }]);
    expect(index.gpu).toEqual(['gpu', 'vram', '顯卡']);
  });
});

describe('matchesSettingsSection', () => {
  it('shows every section for an empty query', () => {
    expect(matchesSettingsSection('   ', 'customize', 'Customize')).toBe(true);
  });

  it('matches Traditional Chinese keywords', () => {
    expect(matchesSettingsSection('歷史', 'customize', '自訂')).toBe(true);
    expect(matchesSettingsSection('隱私', 'data', '資料')).toBe(true);
  });

  it('matches Simplified Chinese keywords', () => {
    expect(matchesSettingsSection('历史', 'customize', '自定义')).toBe(true);
    expect(matchesSettingsSection('显卡', 'gpu', 'GPU 加速')).toBe(true);
  });

  it('matches English keywords under a Chinese title', () => {
    expect(matchesSettingsSection('history', 'customize', '自訂')).toBe(true);
    expect(matchesSettingsSection('GPU', 'gpu', '硬體加速')).toBe(true);
  });

  it('matches the localized title', () => {
    expect(matchesSettingsSection('快捷', 'hotkeys', '快捷鍵')).toBe(true);
  });

  it('does not match unrelated sections', () => {
    expect(matchesSettingsSection('歷史', 'gpu', 'GPU')).toBe(false);
    expect(matchesSettingsSection('zzz-no-such-term', 'about', 'About')).toBe(false);
  });

  it('has a keyword entry for every section', () => {
    const sections = Object.keys(en.settingsSearch);
    const index = buildSearchIndex([en.settingsSearch]);
    for (const section of sections) expect(index[section]?.length).toBeGreaterThan(0);
  });
});
