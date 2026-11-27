// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Localized Docs URL Builder
 *
 * Builds a URL into docs.echophrase.com, prefixed with the current (or
 * given) locale's docs mirror path. The docs site mirrors every page under
 * a per-language prefix, so a blanket prefix is correct for any path -
 * adding a language later is a single LOCALES entry (see $lib/locales/types).
 */

import { WEBSITE_URLS } from '$lib/config/environment';
import { getLocaleInfo, type SupportedLocale } from '$lib/locales';
import { getCurrentLocale } from '$lib/stores/locale.svelte';

/**
 * Build a docs.echophrase.com URL localized for the given (or current) locale.
 *
 * @param path - page path, eg '' for root, '/mcp-server', '/features/custom-models'.
 *   May include a hash fragment, eg '/troubleshooting/permissions#mic'.
 * @param locale - explicit locale to use; defaults to the app's current locale.
 */
export function docsUrl(path: string, locale?: SupportedLocale): string {
  const resolvedLocale = locale ?? getCurrentLocale();
  const docsPath = getLocaleInfo(resolvedLocale).docsPath;
  return `${WEBSITE_URLS.docs}${docsPath}${path}`;
}
