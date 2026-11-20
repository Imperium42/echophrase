<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { isUnsupportedMac } from '$lib/stores/platformCache';
  import { t } from '$lib/locales';
</script>

{#if $isUnsupportedMac}
  <div
    class="fixed top-0 left-0 right-0 z-50 bg-red-950/95 border-b border-red-800/60 px-4 py-3 animate-fade-in-fast"
  >
    <div class="max-w-2xl mx-auto">
      <div class="flex items-start gap-3">
        <span class="text-red-400 text-lg mt-0.5">!</span>
        <div class="text-sm">
          <p class="text-red-300 font-medium mb-1">{t('unsupportedPlatform.title')}</p>
          <p class="text-red-400/80 leading-relaxed">
            {t('unsupportedPlatform.description')}
          </p>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  @keyframes fade-in-fast {
    from {
      opacity: 0;
      transform: translateY(-8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .animate-fade-in-fast {
    animation: fade-in-fast 0.2s ease-out forwards;
  }
</style>
