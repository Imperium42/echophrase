<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { openUpdateModal } from '$lib/stores/updater';
  import { bannersReady } from '$lib/composables/usePageLoadAnimation';
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';
  import BannerShell from './BannerShell.svelte';
</script>

<!-- Top-Left: Update Available Banner -->
{#if $bannersReady}
  <BannerShell>
    <button
      on:click={() => openUpdateModal()}
      class="update-btn group relative overflow-hidden bg-gradient-to-r from-sky-500 via-sky-400 to-blue-500 hover:from-sky-400 hover:via-sky-300 hover:to-blue-400 px-2.5 py-1 rounded-lg text-white text-xs transition-[background] duration-200 ease-out shadow-md shadow-sky-500/25 hover:shadow-lg hover:shadow-sky-500/40 cursor-pointer outline-none focus:outline-none"
    >
      <!-- Shimmer overlay -->
      <div
        class="shimmer absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent"
      ></div>

      <div class="relative flex items-center gap-1">
        <span class="font-medium">{t('update.update')}</span>
        <Icon name="update-available" class="w-3.5 h-3.5" strokeWidth={2.5} />
      </div>
    </button>
  </BannerShell>
{/if}

<style>
  .shimmer {
    transform: translateX(-100%);
  }

  :global(.group:hover) .shimmer {
    animation: shimmer-sweep 0.7s ease-out forwards;
  }
</style>
