// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Environment Configuration
 *
 * Single source of truth for environment detection and the public website
 * URLs the app links out to. The app talks to no server of its own: it only
 * downloads models (Hugging Face) and checks for updates (CrabNebula).
 *
 * Environments:
 * - development: local dev build (website links still point at the public site)
 * - staging / production: echophrase.com
 *
 * Set via VITE_APP_ENV environment variable, or auto-detected from Vite's MODE.
 */

export type AppEnvironment = 'development' | 'staging' | 'production';

/**
 * Detect current environment
 * Priority: VITE_APP_ENV > Vite MODE mapping
 */
function detectEnvironment(): AppEnvironment {
  const envVar = import.meta.env.VITE_APP_ENV as string | undefined;

  if (envVar === 'production' || envVar === 'staging' || envVar === 'development') {
    return envVar;
  }

  // Fallback to Vite's MODE (beta builds use --mode staging)
  if (import.meta.env.MODE === 'production') {
    return 'production';
  }
  if (import.meta.env.MODE === 'staging') {
    return 'staging';
  }

  return 'development';
}

export const APP_ENV: AppEnvironment = detectEnvironment();

/**
 * External website URL (for opening in browser from desktop app).
 * Always points at the public marketing site regardless of env.
 */
export const EXTERNAL_WEBSITE_URL = 'https://echophrase.com';

/**
 * Website page URLs (for external browser navigation)
 */
export const WEBSITE_URLS = {
  home: EXTERNAL_WEBSITE_URL,
  terms: `${EXTERNAL_WEBSITE_URL}/terms`,
  privacy: `${EXTERNAL_WEBSITE_URL}/privacy`,
  download: `${EXTERNAL_WEBSITE_URL}/download`,
  support: `${EXTERNAL_WEBSITE_URL}/contact`,
  /** Base docs host (unlocalized). Use `docsUrl()` from $lib/utils/docsUrl for
   *  any docs page link - it prefixes this with the current locale's mirror path. */
  docs: 'https://docs.echophrase.com',
  discord: 'https://i42.games/discord',
  imperium: 'https://imperium42.com',
} as const;

/** Public source repository (MPL-2.0). */
export const SOURCE_CODE_URL = 'https://github.com/Imperium42/echophrase';

/** GitHub Sponsors page (the project's only funding channel). */
export const SPONSORS_URL = 'https://github.com/sponsors/imperium42';

/**
 * Helper to check environment
 */
export const isDev = APP_ENV === 'development';
export const isStaging = APP_ENV === 'staging';
export const isProduction = APP_ENV === 'production';
