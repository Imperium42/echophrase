// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Locale Store (Svelte 5 Runes)
 *
 * Reactive locale state using $state/$derived.
 * Persisted via Tauri settings and synced across the app.
 *
 * Usage:
 *   import { locale, setLocale, t } from '$lib/stores/locale.svelte';
 *
 *   // Get current locale (reactive in .svelte contexts)
 *   locale.current // 'en' | 'zh-TW' | 'zh-CN'
 *
 *   // Set locale (persists automatically)
 *   setLocale('zh-TW');
 *
 *   // Translate a key (reactive in .svelte contexts)
 *   t('common.save') // "Save" or "儲存"
 */

import {
  type SupportedLocale,
  DEFAULT_LOCALE,
  getLocaleStrings,
  getNestedValue,
  isValidLocale,
} from '$lib/locales/types';

// Reactive locale state
let _locale = $state<SupportedLocale>(DEFAULT_LOCALE);

/**
 * Reactive locale accessor
 *
 * Usage in .svelte files:
 *   locale.current // reactive read
 */
export const locale = {
  get current() {
    return _locale;
  },
};

/**
 * Set the current locale
 *
 * Updates reactive state and persists to Tauri settings.
 */
export function setLocale(newLocale: SupportedLocale): void {
  if (!isValidLocale(newLocale)) {
    console.warn(`[locale] Invalid locale: ${newLocale}, falling back to ${DEFAULT_LOCALE}`);
    newLocale = DEFAULT_LOCALE;
  }

  _locale = newLocale;
  applyHtmlLang(newLocale);

  // Persist to Tauri settings in background (don't block UI)
  import('@tauri-apps/api/core')
    .then(({ invoke }) => invoke('save_language_setting', { language: newLocale }))
    .catch(() => {
      // Not in Tauri context (eg during SSR or testing)
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem('echophrase_locale', newLocale);
      }
    });
}

/**
 * Initialize locale from persisted settings
 *
 * Priority: saved preference > OS locale (Rust) > browser locale > English default.
 * Call this during app initialization (in useAppInit).
 */
export async function initLocale(): Promise<SupportedLocale> {
  let savedLocale: string | null = null;

  // 1. Try saved preference from Tauri settings
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    savedLocale = await invoke<string | null>('get_language_setting');
  } catch {
    if (typeof localStorage !== 'undefined') {
      savedLocale = localStorage.getItem('echophrase_locale');
    }
  }

  if (savedLocale && isValidLocale(savedLocale)) {
    _locale = savedLocale;
    applyHtmlLang(savedLocale);
    return savedLocale;
  }

  // 2. Try OS-level locale detection (Rust backend, cross-platform)
  const osLocale = await detectOsLocale();
  if (osLocale) {
    _locale = osLocale;
    setLocale(osLocale);
    return osLocale;
  }

  // 3. Fall back to browser/webview locale
  const browserLocale = detectBrowserLocale();
  if (browserLocale) {
    _locale = browserLocale;
    setLocale(browserLocale);
    return browserLocale;
  }

  return DEFAULT_LOCALE;
}

/**
 * Detect OS locale via Rust backend (cross-platform).
 */
async function detectOsLocale(): Promise<SupportedLocale | null> {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const osLocale = await invoke<string | null>('get_system_locale');
    if (!osLocale) return null;
    return mapLocaleCode(osLocale);
  } catch {
    return null;
  }
}

/**
 * Set `<html lang>` so CSS `:lang()` font stacks and the webview's text
 * rendering (glyph variants, line breaking) follow the active UI locale.
 * `app.html` hardcodes `lang="en"`; this keeps it in sync at runtime.
 */
function applyHtmlLang(locale: SupportedLocale): void {
  if (typeof document === 'undefined') return;
  document.documentElement.lang = locale;
}

/** Regions whose default written Chinese is Simplified. */
const SIMPLIFIED_CHINESE_REGIONS = ['cn', 'sg', 'my'];

/**
 * Map an OS locale code to a supported locale.
 * Handles platform differences (Windows, macOS, Linux).
 *
 * Chinese classification mirrors the Rust `locale_prefers_simplified_chinese`
 * allowlist (model_selector.rs): an explicit `Hans` script subtag wins, an
 * explicit `Hant` subtag excludes, otherwise region cn/sg/my selects
 * Simplified. Anything else Chinese (zh-TW, zh-HK, zh-MO, bare zh) keeps the
 * Traditional default so an unclassified tag never lands on the wrong script.
 */
export function mapLocaleCode(code: string): SupportedLocale | null {
  if (isValidLocale(code)) return code;

  const normalized = code.split('.')[0].replace(/_/g, '-');
  if (isValidLocale(normalized)) return normalized;

  const subtags = normalized.toLowerCase().split('-');
  const base = subtags[0];

  if (base === 'en') return 'en';

  if (base === 'zh') {
    if (subtags.includes('hans')) return 'zh-CN';
    if (subtags.includes('hant')) return 'zh-TW';
    if (subtags.some(tag => SIMPLIFIED_CHINESE_REGIONS.includes(tag))) return 'zh-CN';
    return 'zh-TW';
  }

  return null;
}

/**
 * Detect browser/webview locale from navigator.languages
 */
function detectBrowserLocale(): SupportedLocale | null {
  if (typeof navigator === 'undefined') return null;

  const browserLocales = navigator.languages || [navigator.language];

  for (const browserLocale of browserLocales) {
    const mapped = mapLocaleCode(browserLocale);
    if (mapped) return mapped;
  }

  return null;
}

/**
 * Reactive translation function
 *
 * Reads $state internally - reactive when called in .svelte template contexts.
 *
 * Usage in .svelte files:
 *   {t('common.save')} // reactive
 *   {t('settings.noResults', { query: 'foo' })} // with params
 */
export function t(key: string, params?: Record<string, string>): string {
  const strings = getLocaleStrings(_locale);
  return getNestedValue(strings as unknown as Record<string, unknown>, key, params);
}

/**
 * Get the current locale synchronously (non-reactive)
 *
 * Use in non-Svelte contexts (services, utilities).
 */
export function getCurrentLocale(): SupportedLocale {
  return _locale;
}

/**
 * Get translation synchronously (non-reactive)
 *
 * Useful in non-Svelte contexts (services, utilities).
 */
export function translate(key: string, params?: Record<string, string>): string {
  const strings = getLocaleStrings(_locale);
  return getNestedValue(strings as unknown as Record<string, unknown>, key, params);
}
