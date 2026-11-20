<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';

  /** Whether a download is actively in progress */
  export let isDownloadActive: boolean;
  /** Whether the full download modal is open */
  export let isModalOpen: boolean;
  /** Download progress 0-100 */
  export let progress: number;
  /** Download speed string (e.g., "5.2 MB/s") */
  export let speed: string;
  /** Time remaining string */
  export let timeRemaining: string;
  /** Callback to open the full download modal */
  export let onOpenModal: () => void;
</script>

<!-- Mini Download Progress (bottom-center, above model selector) - shown when downloading in background with modal closed -->
{#if isDownloadActive && !isModalOpen}
  <button
    class="fixed bottom-14 left-1/2 -translate-x-1/2 w-44 z-[160] cursor-pointer group"
    on:click={onOpenModal}
    title={t('miniDownload.clickToOpen')}
  >
    <div
      class="bg-zinc-800/90 backdrop-blur-sm rounded-lg p-2.5 border border-cyan-500/30 shadow-lg transition-all duration-200 group-hover:border-cyan-400/50 group-hover:bg-zinc-700/90"
    >
      <div class="text-xs text-cyan-400 mb-1.5 flex items-center justify-between gap-2">
        <div class="flex items-center gap-1.5 min-w-0 flex-1">
          <Icon name="download" class="w-3 h-3 flex-shrink-0" />
          <span class="truncate">{t('miniDownload.downloading')}</span>
        </div>
        <span class="font-mono font-bold flex-shrink-0">{Math.round(progress)}%</span>
      </div>
      <div class="h-2 bg-zinc-700 rounded-full overflow-hidden relative mini-water-glow">
        <div
          class="absolute inset-0 water-bar rounded-full"
          style="clip-path: inset(0 {100 -
            progress}% 0 0 round 9999px); transition: clip-path 700ms cubic-bezier(0.33, 1, 0.68, 1)"
        >
          <div class="absolute inset-0 overflow-hidden">
            <div class="absolute inset-y-0 w-full water-undercurrent"></div>
          </div>
          <div class="absolute inset-0 overflow-hidden">
            <div class="absolute inset-y-0 w-full water-shimmer"></div>
          </div>
        </div>
      </div>
      {#if speed}
        <div class="text-xs text-zinc-500 mt-1 flex items-center gap-1">
          <Icon name="speed" class="w-3 h-3" />
          <span>{speed}</span>
          {#if timeRemaining}
            <span class="text-zinc-600">·</span>
            <span>{t('miniDownload.timeLeft', { time: timeRemaining })}</span>
          {/if}
        </div>
      {/if}
    </div>
  </button>
{/if}
