// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Banner Visibility Store
 *
 * Temporarily hides the top-left banners (Beta / Update) so a
 * clean window can be recorded for a stream or promo video. Driven by the
 * "Hide Banners 5m" debug button; F4 cancels early, and the window self-expires.
 *
 * Applied once in `BannerShell.svelte`, which every top-left banner wraps.
 *
 * DEV-only on purpose: `enabled` is gated so a leftover localStorage key can
 * never suppress a real Update or Beta-ended banner in a production build.
 */

import { createTimedHideStore } from './timedHide';

export const bannersHidden = createTimedHideStore({
  key: 'banner-hide-until',
  label: 'Top-left banners',
  enabled: import.meta.env.DEV,
});
