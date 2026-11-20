// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, expect, it } from 'vitest';
import en from './en.json';
import zhTW from './zh-TW.json';
import zhCN from './zh-CN.json';
import { mapLocaleCode } from '$lib/stores/locale.svelte';

type Leaf = { key: string; value: string };

/** Flatten a nested locale object to dot-path string leaves. */
function flatten(obj: unknown, prefix = ''): Leaf[] {
  if (typeof obj === 'string') return [{ key: prefix, value: obj }];
  if (Array.isArray(obj)) {
    return obj.flatMap((v, i) => flatten(v, `${prefix}[${i}]`));
  }
  if (obj && typeof obj === 'object') {
    return Object.entries(obj).flatMap(([k, v]) => flatten(v, prefix ? `${prefix}.${k}` : k));
  }
  return [];
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort();
}

const locales = { 'zh-TW': zhTW, 'zh-CN': zhCN } as const;
const enLeaves = flatten(en);
const enKeys = enLeaves.map(l => l.key);

describe('locale parity', () => {
  for (const [code, data] of Object.entries(locales)) {
    it(`${code} has exactly the same keys as en`, () => {
      const keys = flatten(data).map(l => l.key);
      expect(keys.filter(k => !enKeys.includes(k))).toEqual([]);
      expect(enKeys.filter(k => !keys.includes(k))).toEqual([]);
    });

    it(`${code} keeps every {placeholder} that en uses`, () => {
      const byKey = new Map(flatten(data).map(l => [l.key, l.value]));
      const mismatches = enLeaves
        .filter(l => placeholders(l.value).length > 0)
        .filter(l => placeholders(byKey.get(l.key) ?? '').join() !== placeholders(l.value).join())
        .map(l => l.key);
      expect(mismatches).toEqual([]);
    });

    it(`${code} contains no em/en dashes`, () => {
      const offenders = flatten(data)
        .filter(l => /[–—]/.test(l.value))
        .map(l => l.key);
      expect(offenders).toEqual([]);
    });
  }
});

describe('mapLocaleCode', () => {
  const cases: Array<[string, string | null]> = [
    ['en', 'en'],
    ['en-US', 'en'],
    ['en_GB.UTF-8', 'en'],
    ['zh-TW', 'zh-TW'],
    ['zh_TW.UTF-8', 'zh-TW'],
    ['zh-HK', 'zh-TW'],
    ['zh-Hant', 'zh-TW'],
    ['zh-Hant-SG', 'zh-TW'],
    ['zh', 'zh-TW'],
    ['zh-CN', 'zh-CN'],
    ['zh_CN.UTF-8', 'zh-CN'],
    ['zh-Hans', 'zh-CN'],
    ['zh-Hans-SG', 'zh-CN'],
    ['zh-Hans-TW', 'zh-CN'],
    ['zh-SG', 'zh-CN'],
    ['zh-MY', 'zh-CN'],
    ['ja-JP', null],
    ['fr', null],
  ];

  for (const [input, expected] of cases) {
    it(`${input} -> ${expected}`, () => {
      expect(mapLocaleCode(input)).toBe(expected);
    });
  }
});
