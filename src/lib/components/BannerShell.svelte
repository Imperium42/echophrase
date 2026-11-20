<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Shared scaffolding for the top-left banners
   * (UpgradeBanner / BetaBanner / LoginBanner / UpdateBanner).
   *
   * Owns:
   * - The fixed positioning + entrance fade (.banner-entrance, defined globally
   *   in app.css).
   * - The "deferred interactivity" guard: the entire banner is
   *   `pointer-events: none` until the entrance animation finishes, so it
   *   doesn't absorb a click landing in the corner during the fade-in.
   *
   * Banner content (the actual button + tooltip + extras) goes in the slot.
   */

  import { bannersHidden } from '$lib/stores/bannerVisibility';
  import { demoSession } from '$lib/stores/demo';

  /** z-index. UpgradeBanner/BetaBanner use 40 (sit under header dropdowns); LoginBanner/UpdateBanner use 50. */
  export let zIndex: 40 | 50 = 50;
  /** Extra classes appended to the wrapper (e.g. "flex items-center gap-3"). */
  export let extraClass: string = '';

  let interactive = false;
</script>

<!-- $bannersHidden is the dev-only "Hide Banners 5m" window (streaming/promo
     capture). The store is hard-gated to import.meta.env.DEV, so this is always
     false in production builds. -->
{#if !$bannersHidden && !$demoSession}
  <div
    class="fixed top-4 left-4 banner-entrance {extraClass}"
    style:z-index={zIndex}
    class:pointer-events-none={!interactive}
    on:animationend={() => (interactive = true)}
  >
    <slot />
  </div>
{/if}
