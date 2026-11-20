<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Transcription Preview Area
   *
   * Shows transcription text on the right side of the record button.
   * - Realtime preview bubble during/after recording (zinc themed).
   * - Amber "projection bubble" driven by $rightProjection — appears when the
   *   user hovers a left-side chip to surface its content on this side.
   */
  import { onDestroy } from 'svelte';
  import type { Readable } from 'svelte/store';
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';
  import { modalController } from '$lib/stores/modalController';
  import { rightProjection } from '$lib/stores/rightProjection';
  import { pageStore } from '$lib/stores/pageStore';
  import { platformCache } from '$lib/stores/platformCache';

  /** The current realtime text to display */
  export let realtimeText: Readable<string>;
  /** Whether realtime preview is visible */
  export let realtimeVisible: Readable<boolean>;
  /** Whether realtime preview is fading */
  export let realtimeFading: Readable<boolean>;
  /** Current page index for animation */
  export let realtimePageIndex: Readable<number>;
  /** Whether showing original text before transform */
  export let isShowingOriginal: Readable<boolean>;
  /** Whether transform animation is in progress */
  export let isTransforming: Readable<boolean>;
  /** True when the current result is the backend's silence sentinel (near-silent
   * audio, or a known ASR hallucination on near-silent audio) */
  export let isSilenceResult: Readable<boolean>;

  function openMicSettings() {
    pageStore.openSettings('audio-input');
  }

  const MAX_PREVIEW_LENGTH = 280;

  let bubbleHovered = false;
  let expandedOpen = false;
  let expandedText = '';
  let modalId: string | null = null;

  // Word splitting for animation
  $: realtimeIsTruncated = $realtimeText.length > MAX_PREVIEW_LENGTH;
  $: realtimePreviewText = realtimeIsTruncated
    ? $realtimeText.slice(0, MAX_PREVIEW_LENGTH) + '...'
    : $realtimeText;
  $: realtimePreviewWords = realtimePreviewText
    .trim()
    .split(' ')
    .filter(w => w);

  // Scale font-size down for longer transcripts before falling back to scrollbar/popover.
  function sizeClassFor(len: number): string {
    if (len > 220) return 'text-xs';
    if (len > 140) return 'text-[0.8125rem]';
    return 'text-sm';
  }
  $: previewSizeClass = sizeClassFor(realtimePreviewText.length);
  $: projectionSizeClass = sizeClassFor(($rightProjection?.text ?? '').length);

  function openExpanded(text: string) {
    expandedText = text;
    expandedOpen = true;
    if (modalId) modalController.pop(modalId);
    modalId = modalController.push({ onClose: closeExpanded, priority: 0 });
    setTimeout(() => document.addEventListener('click', handlePopoverClickOutside), 0);
  }

  function closeExpanded() {
    expandedOpen = false;
    expandedText = '';
    if (modalId) {
      modalController.pop(modalId);
      modalId = null;
    }
    document.removeEventListener('click', handlePopoverClickOutside);
  }

  function handlePopoverClickOutside(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest('.preview-expanded') && !target.closest('.preview-bubble')) {
      closeExpanded();
    }
  }

  onDestroy(() => {
    if (modalId) modalController.pop(modalId);
    document.removeEventListener('click', handlePopoverClickOutside);
  });
</script>

<!-- Right-side display surface. No hover handlers here — pure projection target. -->
<div
  class="absolute top-full left-1/2 -translate-x-1/2 mt-2"
  style="width: clamp(16rem, calc(100vw / var(--ui-zoom, 1) - 9rem), 32rem);"
  role="region"
  aria-label={t('transcription.title')}
>
  <!-- Content container -->
  <div class="relative">
    <!-- Realtime preview (during/after recording) -->
    {#if $realtimeVisible && $realtimeText}
      <div
        class="absolute left-2 top-0 min-w-0 max-w-[calc(100%-1rem)] w-[calc(100%-1rem)] transition-all duration-300 ease-out {$realtimeFading
          ? 'opacity-0 scale-95'
          : 'opacity-100 scale-100'}"
      >
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div
          class="preview-bubble bg-zinc-800/70 backdrop-blur-sm px-4 py-3 rounded-lg border shadow-lg transition-all duration-500
            {$isShowingOriginal ? 'border-zinc-700/40' : ''}
            {$isTransforming
            ? 'border-purple-500/50 shadow-purple-500/10 animate-transform-glow'
            : 'border-zinc-700/40'}
            {realtimeIsTruncated && !$isTransforming ? 'hover:bg-zinc-700/70 cursor-pointer' : ''}"
          onmouseenter={() => (bubbleHovered = true)}
          onmouseleave={() => (bubbleHovered = false)}
          onclick={() => realtimeIsTruncated && openExpanded($realtimeText)}
          onkeydown={e => e.key === 'Enter' && realtimeIsTruncated && openExpanded($realtimeText)}
          role={realtimeIsTruncated ? 'button' : 'status'}
          tabindex={realtimeIsTruncated ? 0 : -1}
        >
          {#if $isSilenceResult}
            <!-- Dedicated silence message: not the raw sentinel text, and not
                 word-animated (it's a UI message, not a transcript) so the
                 "mic settings" phrase can stay a real clickable link. -->
            <p
              class="preview-text {previewSizeClass} font-normal leading-relaxed text-center break-words text-zinc-400"
            >
              {t('audioInput.noSoundDetectedPrefix')}<button
                type="button"
                class="silence-settings-link"
                onclick={openMicSettings}>{t('audioInput.micSettingsLink')}</button
              >{t('audioInput.noSoundDetectedSuffix')}
              {#if platformCache.isWsl}
                <br />{t('audioInput.wslPassthroughTooltip')}
              {/if}
            </p>
          {:else}
            <p
              class="preview-text {previewSizeClass} font-normal leading-relaxed text-center break-words transition-all duration-500
              {$isShowingOriginal ? 'text-zinc-300/90' : ''}
              {$isTransforming ? 'text-purple-300 animate-transform-text' : ''}
              {bubbleHovered && realtimeIsTruncated ? 'text-zinc-100' : 'text-zinc-300/90'}"
            >
              {#each realtimePreviewWords as word, i ($realtimePageIndex + '-' + i)}
                <span class="inline-block animate-word-fade-in" style="animation-delay: {i * 30}ms"
                  >{word}</span
                >{' '}
              {/each}
            </p>
          {/if}
        </div>
      </div>
    {/if}

    <!-- Expanded panel: in-place, full-text, scrollable. Rendered as a card-level
         sibling so it survives the realtime block unmounting (the parent clears
         the store after fade); text is snapshotted into expandedText. -->
    {#if expandedOpen}
      <div
        class="preview-expanded absolute left-2 right-2 top-0 z-50 bg-zinc-900 border border-zinc-700/50 rounded-lg shadow-xl animate-popover-in"
      >
        <div class="flex items-center justify-between px-3 py-2 border-b border-zinc-800">
          <span class="text-xs font-medium text-zinc-400">{t('transcription.title')}</span>
          <button
            type="button"
            class="text-zinc-500 hover:text-zinc-300 transition-colors"
            onclick={closeExpanded}
            aria-label={t('common.close')}
          >
            <Icon name="close" class="w-3.5 h-3.5" />
          </button>
        </div>
        <div class="p-3">
          <div
            class="preview-text text-sm font-normal text-zinc-300 whitespace-pre-wrap break-words max-h-[42vh] overflow-y-auto custom-scrollbar"
          >
            {expandedText}
          </div>
        </div>
      </div>
    {/if}

    <!-- Left-chip projection bubble (amber). Wrapper stays mounted whenever
         the realtime preview isn't showing, so chip-to-chip hovers just swap
         content inside with a cheap CSS transition — no keyframe restart. -->
    {#if !$realtimeVisible}
      <div
        class="projection-wrap absolute left-2 top-0 min-w-0 max-w-[calc(100%-1rem)] w-[calc(100%-1rem)]"
        class:projection-visible={!!$rightProjection}
        aria-hidden={!$rightProjection}
      >
        <div
          class="bg-amber-950/30 backdrop-blur-sm px-4 py-3 rounded-lg border border-amber-700/20 shadow-lg"
        >
          <div class="text-[10px] uppercase tracking-wider font-medium text-amber-400/70 mb-1">
            {$rightProjection?.label ?? ''}
          </div>
          <p
            class="preview-text {projectionSizeClass} font-normal leading-relaxed text-left whitespace-pre-wrap break-words text-amber-200/90 max-h-[32vh] overflow-y-auto custom-scrollbar"
          >
            {$rightProjection?.text ?? ''}
          </p>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  @keyframes word-fade-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  @keyframes transform-glow {
    0% {
      box-shadow: 0 0 0 0 rgba(168, 85, 247, 0);
    }
    50% {
      box-shadow: 0 0 20px 4px rgba(168, 85, 247, 0.3);
    }
    100% {
      box-shadow: 0 4px 6px -1px rgba(168, 85, 247, 0.1);
    }
  }

  @keyframes transform-text {
    0% {
      filter: blur(0);
      opacity: 1;
    }
    30% {
      filter: blur(2px);
      opacity: 0.7;
    }
    60% {
      filter: blur(0);
      opacity: 1;
    }
    100% {
      filter: blur(0);
      opacity: 1;
    }
  }

  @keyframes popover-in {
    from {
      opacity: 0;
      transform: scale(0.95) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  /* Projection bubble: GPU-accelerated opacity/translate, no keyframe restart
     on content swap. 120ms in-out so chip-to-chip hops feel instant.
     `visibility: hidden` after fade-out so the backdrop-blur stops painting
     when the bubble isn't shown. */
  .projection-wrap {
    opacity: 0;
    transform: translate3d(-6px, 0, 0);
    visibility: hidden;
    pointer-events: none;
    will-change: opacity, transform;
    transition:
      opacity 120ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 120ms cubic-bezier(0.4, 0, 0.2, 1),
      visibility 0s linear 120ms;
  }

  .projection-wrap.projection-visible {
    opacity: 1;
    transform: translate3d(0, 0, 0);
    visibility: visible;
    pointer-events: auto;
    transition:
      opacity 120ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 120ms cubic-bezier(0.4, 0, 0.2, 1),
      visibility 0s linear 0s;
  }

  .animate-word-fade-in {
    animation: word-fade-in 0.15s ease-out forwards;
    opacity: 0;
  }

  .animate-transform-glow {
    animation: transform-glow 0.6s ease-out forwards;
  }

  .animate-transform-text {
    animation: transform-text 0.6s ease-out forwards;
  }

  .animate-popover-in {
    animation: popover-in 0.15s ease-out;
  }

  .silence-settings-link {
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    font: inherit;
    color: rgb(132 204 22 / 0.9);
    text-decoration: underline;
    text-decoration-color: rgb(132 204 22 / 0.4);
    cursor: pointer;
  }

  .silence-settings-link:hover {
    color: rgb(132 204 22);
    text-decoration-color: rgb(132 204 22 / 0.8);
  }

  .preview-text {
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    text-rendering: optimizeLegibility;
    font-feature-settings:
      'kern' 1,
      'liga' 1;
  }

  .custom-scrollbar {
    scrollbar-width: thin;
    scrollbar-color: rgba(161, 161, 170, 0.3) transparent;
  }

  .custom-scrollbar::-webkit-scrollbar {
    width: 6px;
  }

  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(161, 161, 170, 0.3);
    border-radius: 3px;
  }

  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: rgba(161, 161, 170, 0.5);
  }
</style>
