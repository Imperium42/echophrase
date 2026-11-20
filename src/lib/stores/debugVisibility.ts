// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Debug Visibility Store
 *
 * Controls visibility of debug sections across the app.
 * Useful for hiding dev tools during streaming.
 *
 * Sibling: `bannerVisibility.ts` does the same for the top-left banners.
 */

import { createTimedHideStore } from './timedHide';

export const debugHidden = createTimedHideStore({
  key: 'debug-section-hide-until',
  label: 'Debug sections',
  // The panel only renders under import.meta.env.DEV anyway; gating here keeps a
  // stale localStorage key from ever mattering in a production build.
  enabled: import.meta.env.DEV,
});
