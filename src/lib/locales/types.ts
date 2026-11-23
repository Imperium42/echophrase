// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Localization Types & Utilities
 *
 * Separated from index.ts to avoid circular imports with the locale store.
 */

import en from './en.json';
import zhTW from './zh-TW.json';
import zhCN from './zh-CN.json';

// Type inference from English locale (source of truth)
export type LocaleStrings = typeof en;
export type LocaleKey = keyof LocaleStrings;

// Supported locale codes (ISO 639-1 + region)
export type SupportedLocale = 'en' | 'zh-TW' | 'zh-CN';

// Locale metadata for UI display
export interface LocaleInfo {
  code: SupportedLocale;
  name: string; // Native name (eg "繁體中文")
  englishName: string; // English name (eg "Traditional Chinese")
  flag?: string; // Optional emoji flag
  beta?: boolean; // Show BETA tag for incomplete translations
  /** URL prefix on docs.echophrase.com for this language, '' for English */
  docsPath: string;
}

/**
 * Available locales with display metadata
 */
export const LOCALES: LocaleInfo[] = [
  { code: 'en', name: 'English', englishName: 'English', docsPath: '' },
  {
    code: 'zh-TW',
    name: '繁體中文',
    englishName: 'Traditional Chinese',
    beta: true,
    docsPath: '/zh-Hant',
  },
  {
    code: 'zh-CN',
    name: '简体中文',
    englishName: 'Simplified Chinese',
    beta: true,
    docsPath: '/zh-CN',
  },
];

/**
 * Default locale (used when no preference is set)
 */
export const DEFAULT_LOCALE: SupportedLocale = 'en';

/**
 * Locale data map
 */
const localeData: Record<SupportedLocale, LocaleStrings> = {
  en: en,
  'zh-TW': zhTW,
  'zh-CN': zhCN,
};

/**
 * Get locale strings for a given locale code
 */
export function getLocaleStrings(locale: SupportedLocale): LocaleStrings {
  return localeData[locale] ?? localeData[DEFAULT_LOCALE];
}

/**
 * Check if a locale code is supported
 */
export function isValidLocale(code: string): code is SupportedLocale {
  return code in localeData;
}

/**
 * Get display/metadata info for a given locale code
 */
export function getLocaleInfo(code: SupportedLocale): LocaleInfo {
  return LOCALES.find(l => l.code === code) ?? LOCALES.find(l => l.code === DEFAULT_LOCALE)!;
}

/**
 * Get nested value from locale object using dot notation
 */
export function getNestedValue(
  obj: Record<string, unknown>,
  path: string,
  params?: Record<string, string>
): string {
  const keys = path.split('.');
  let value: unknown = obj;

  for (const key of keys) {
    if (value && typeof value === 'object' && key in value) {
      value = (value as Record<string, unknown>)[key];
    } else {
      console.warn(`[i18n] Missing translation key: ${path}`);
      return path;
    }
  }

  if (typeof value !== 'string') {
    console.warn(`[i18n] Translation key "${path}" is not a string`);
    return path;
  }

  if (params) {
    return value.replace(/\{(\w+)\}/g, (_, key) => params[key] ?? `{${key}}`);
  }

  return value;
}

/**
 * ISO language code for API requests
 */
export function getApiLanguageCode(locale: SupportedLocale): string | undefined {
  if (locale === 'en') return undefined;
  return locale;
}
