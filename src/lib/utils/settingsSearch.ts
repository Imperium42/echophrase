// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Settings search keywords.
 *
 * Each section's keywords live in the locale files under `settingsSearch.<sectionKey>`
 * as one comma-separated string. A query matches a section if it is a substring of the
 * section's localized title or of any keyword from ANY locale, so English terms ("gpu",
 * "history") work in a Chinese UI and Simplified terms work in a Traditional UI.
 * Substring matching needs no word boundaries, so it works for CJK too.
 */
import en from '$lib/locales/en.json';
import zhTW from '$lib/locales/zh-TW.json';
import zhCN from '$lib/locales/zh-CN.json';

type KeywordTable = Record<string, string>;

/** Split a comma-separated keyword string into trimmed, lowercased terms. */
export function parseTerms(csv: string | undefined): string[] {
  if (!csv) return [];
  return csv
    .split(',')
    .map(term => term.trim().toLowerCase())
    .filter(Boolean);
}

/** Merge keyword tables into sectionKey -> unique terms. */
export function buildSearchIndex(tables: KeywordTable[]): Record<string, string[]> {
  const index: Record<string, string[]> = {};
  for (const table of tables) {
    for (const [section, csv] of Object.entries(table)) {
      const merged = new Set([...(index[section] ?? []), ...parseTerms(csv)]);
      index[section] = [...merged];
    }
  }
  return index;
}

const SEARCH_INDEX = buildSearchIndex([
  en.settingsSearch,
  zhTW.settingsSearch,
  zhCN.settingsSearch,
]);

/** True when `query` is empty or matches the section's title or keywords. */
export function matchesSettingsSection(
  query: string,
  sectionKey: string,
  sectionTitle: string,
  index: Record<string, string[]> = SEARCH_INDEX
): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  if (sectionTitle.toLowerCase().includes(q)) return true;
  return (index[sectionKey] ?? []).some(term => term.includes(q));
}
