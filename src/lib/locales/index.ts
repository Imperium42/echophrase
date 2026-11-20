// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Localization Module
 *
 * Provides type-safe internationalization for Echophrase.
 *
 * Supported locales:
 * - en: English (default)
 * - zh-TW: Traditional Chinese (繁體中文)
 *
 * Usage:
 *   import { t, locale, setLocale } from '$lib/locales';
 *   const text = t('common.save'); // "Save" or "儲存"
 */

// Re-export types and utilities
export {
  type LocaleStrings,
  type LocaleKey,
  type SupportedLocale,
  type LocaleInfo,
  LOCALES,
  DEFAULT_LOCALE,
  getLocaleStrings,
  isValidLocale,
  getNestedValue,
  getApiLanguageCode,
  getLocaleInfo,
} from './types';

// Re-export locale store and translation function from runes-based store
export {
  locale,
  setLocale,
  t,
  getCurrentLocale,
  translate,
  initLocale,
} from '$lib/stores/locale.svelte';
