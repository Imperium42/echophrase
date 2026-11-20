<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Shared "downloading" view: water-bar progress + stats + Cancel/Pause,
   * used by BOTH the registry model picker (ModelDownloadProgress.svelte)
   * and the BYO custom-model import modal (ImportCustomModelModal.svelte).
   *
   * Extracted after the two flows drifted despite already sharing
   * ModelDownloadActions for just the button - the button alone wasn't
   * enough to prevent the surrounding progress UI from diverging. One
   * component means the two flows can no longer drift apart silently.
   * Markup ported verbatim from ModelDownloadProgress.svelte's original
   * Downloading viewState so the registry flow's look/animations are
   * unchanged, and BYO now matches it exactly instead of a lookalike.
   */
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';
  import ModelDownloadActions from './ModelDownloadActions.svelte';

  interface Props {
    /** 0-100 */
    progress: number;
    speed?: string;
    timeRemaining?: string;
    /** Status label shown before progress reaches 1% (falls back to "Connecting...") */
    status?: string;
    /** Known total size, for the "X / Y MB" display. Omit to show just the percentage. */
    totalBytes?: number;
    /** True during the brief post-100% completion animation. */
    isCompleting?: boolean;
    isFadingOut?: boolean;
    onCancel: () => void;
    /** Dedicated minimize icon (registry dialog's top-right button). Omit
     * when the caller's own modal chrome already provides a working
     * close/minimize affordance (eg ImportCustomModelModal's ModalWindow X). */
    onMinimize?: () => void;
    /** "Open download folder" button, next to the progress bar (not the
     * header - that collided with the minimize button above it). Omit to
     * not show a folder button at all. */
    onOpenDir?: () => void;
    openDirLabel?: string;
  }

  let {
    progress,
    speed = '',
    timeRemaining = '',
    status = '',
    totalBytes = 0,
    isCompleting = false,
    isFadingOut = false,
    onCancel,
    onMinimize,
    onOpenDir,
    openDirLabel = '',
  }: Props = $props();
</script>

{#if onMinimize}
  <button
    onclick={onMinimize}
    class="absolute top-4 right-4 w-8 h-8 flex items-center justify-center rounded-lg transition-all ease-out duration-200 hover:bg-zinc-500/10 hover:scale-105 active:brightness-90 text-zinc-400 hover:text-zinc-200 cursor-pointer"
    aria-label={t('modelDownload.minimize')}
    type="button"
  >
    <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor">
      <path stroke-linecap="round" stroke-width="2" d="M5 10H15" />
    </svg>
  </button>
{/if}

<div class="space-y-4">
  <!-- Progress Track -->
  <div class="relative pt-6">
    <div class="flex items-center gap-2">
      <div
        class="water-bar-container rounded-full flex-1 {isCompleting
          ? 'water-complete-glow'
          : ''} {isFadingOut ? 'water-bar-fade-out' : ''}"
      >
        <div class="w-full h-3 bg-zinc-800 rounded-full overflow-hidden relative">
          <!-- Full-width water animation, clipped by progress (GPU-composited, no relayout).
					     Animated gradient layers below already give the water character - no SVG
					     displacement filter (it warped the bar's top/bottom into visible ripples
					     at this height). -->
          <div
            class="absolute inset-0 water-bar rounded-full {isCompleting
              ? 'water-bar-complete'
              : ''}"
            style="clip-path: inset(0 {100 -
              progress}% 0 0 round 9999px); transition: clip-path 700ms cubic-bezier(0.33, 1, 0.68, 1)"
          >
            {#if !isCompleting}
              <div class="absolute inset-0 overflow-hidden">
                <div class="absolute inset-y-0 w-full water-undercurrent"></div>
              </div>
              <div class="absolute inset-0 overflow-hidden">
                <div class="absolute inset-y-0 w-full water-surface"></div>
              </div>
              <div class="absolute inset-0 overflow-hidden">
                <div class="absolute inset-y-0 w-full water-shimmer"></div>
              </div>
            {/if}
          </div>
        </div>
      </div>

      {#if onOpenDir}
        <button
          type="button"
          onclick={onOpenDir}
          title={openDirLabel}
          aria-label={openDirLabel}
          class="flex-shrink-0 w-6 h-6 flex items-center justify-center rounded-md transition-all ease-out duration-200 hover:bg-zinc-500/15 hover:scale-105 active:brightness-90 text-zinc-400 hover:text-zinc-200 cursor-pointer"
        >
          <Icon name="folder" size={14} />
        </button>
      {/if}
    </div>

    <!-- Progress percentage or status label -->
    <div
      class="absolute right-0 top-0 font-mono text-sm font-bold {progress >= 1 && progress <= 99
        ? 'text-cyan-400'
        : 'text-zinc-400'}"
    >
      {#if progress >= 1 && progress <= 99}
        {Math.round(progress)}%
      {:else if status}
        {status}
      {:else}
        {t('recording.connecting')}
      {/if}
    </div>
  </div>

  <!-- Stats -->
  <div class="flex justify-between text-sm text-zinc-400">
    <span class="flex items-center gap-1">
      {#if totalBytes > 0 && progress >= 1}
        {Math.round(((progress / 100) * totalBytes) / 1_000_000)} / {Math.round(
          totalBytes / 1_000_000
        )} MB
        {#if speed && progress < 100}
          <span class="text-zinc-500">·</span>
          {speed}
        {/if}
      {:else if speed && progress < 100}
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        {speed}
      {/if}
    </span>
    <span class="flex items-center gap-1">
      {#if timeRemaining && progress < 100}
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
          />
        </svg>
        {t('modelDownload.eta', { time: timeRemaining })}
      {/if}
    </span>
  </div>

  <ModelDownloadActions {progress} {onCancel} />
</div>
