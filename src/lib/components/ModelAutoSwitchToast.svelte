<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { modelAutoSwitchToast } from '$lib/stores/modelAutoSwitchToast';
  import { getModelById } from '$lib/types/models';
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';

  const modelName = (id: string) => getModelById(id)?.name ?? id;
</script>

{#if $modelAutoSwitchToast}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center pt-16 pointer-events-none"
    transition:fade={{ duration: 200 }}
  >
    <div
      class="pointer-events-auto mx-4 w-full max-w-sm"
      in:fly={{ y: -12, duration: 300 }}
      out:fly={{ y: -8, duration: 200 }}
    >
      <div
        class="rounded-xl border border-amber-500/40 bg-zinc-900/95 shadow-xl shadow-black/40 backdrop-blur-md overflow-hidden"
      >
        <div class="h-0.5 w-full bg-gradient-to-r from-amber-500 via-yellow-400 to-amber-500"></div>
        <div class="flex items-center gap-3 px-4 py-3.5">
          <Icon name="cpu" size={20} class="text-amber-400 shrink-0" />
          <div class="min-w-0">
            <p class="text-sm font-semibold text-amber-300">{t('modelAutoSwitch.title')}</p>
            <p class="text-xs text-zinc-400 mt-0.5">
              {t('modelAutoSwitch.body')
                .replace('{from}', modelName($modelAutoSwitchToast.from))
                .replace('{to}', modelName($modelAutoSwitchToast.to))}
            </p>
          </div>
          <button
            class="ml-auto shrink-0 text-zinc-500 hover:text-zinc-300 transition-colors"
            aria-label={t('modelAutoSwitch.dismiss')}
            onclick={() => modelAutoSwitchToast.dismiss()}
          >
            <Icon name="close" size={16} />
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
