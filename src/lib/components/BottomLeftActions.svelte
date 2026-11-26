<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { t } from '$lib/locales';
  import { useFadeOnIdle } from '$lib/composables/useFadeOnIdle';
  import { useCopyFeedback } from '$lib/composables/useCopyFeedback';
  import { useHotkeys } from '$lib/composables/useHotkeys';
  import { tauriSettings } from '$lib/composables/useTauriSettings';
  import { bottomLeftReady } from '$lib/composables/usePageLoadAnimation';
  import { transcription } from '$lib/stores/transcription';
  import { pageStore } from '$lib/stores/pageStore';
  import { requestHistoryPeek } from '$lib/stores/lastRun';
  import { onMount, onDestroy } from 'svelte';
  import { Icon } from '$lib/icons';

  /** Whether recording is in progress */
  export let isRecording: boolean;
  /** Whether processing is in progress */
  export let isProcessing: boolean;
  /** Whether open mic mode is active (recording stays true, buttons should still show) */
  export let isOpenMicActive: boolean = false;
  /** Callback when copy-last hover state changes */
  export let onHistoryHoverChange: ((hovering: boolean) => void) | undefined = undefined;

  // Reactive settings
  $: showCopyLastButton = $tauriSettings?.show_copy_last_button !== false;
  $: showInputChanger = $tauriSettings?.show_input_changer !== false;
  $: showHotkeyChanger = $tauriSettings?.show_hotkey_changer !== false;

  // Copy last button - extract stores from composable
  const copyLastFade = useFadeOnIdle({ delay: 5000 });
  const copyLastFading = copyLastFade.fading;
  const copyLastHovered = copyLastFade.hovered;

  const copyFeedback = useCopyFeedback();
  const isCopied = copyFeedback.isCopied;

  // Local hover tracking for history-amber theming across all left buttons
  let isHistoryHovering = false;

  // Trigger fade when transcription result changes
  $: if ($transcription.currentResult?.text) {
    copyLastFade.startFade();
  }

  async function copyLastTranscription() {
    const lastText = $transcription.currentResult?.text;
    if (!lastText) return;
    await copyFeedback.copy(lastText);
  }

  // Input changer button - extract stores from composable
  const inputChangerFade = useFadeOnIdle({ delay: 5000 });
  const inputChangerFading = inputChangerFade.fading;
  const inputChangerHovered = inputChangerFade.hovered;

  // Opens Settings filtered to "input" so the mic picker is right there
  function openRecordingSettings() {
    pageStore.openSettingsWithSearch('input');
  }

  // Hotkey button: opens Settings filtered to "hotkeys", like the mic button does
  // for "input". The one hotkey recorder lives in Settings > Hotkeys.
  const hotkeyFade = useFadeOnIdle({ delay: 5000 });
  const hotkeyFading = hotkeyFade.fading;
  const hotkeyHovered = hotkeyFade.hovered;

  // The saved hotkey (not the recorder's live label, so a capture running in
  // Settings doesn't flash "Press keys..." on this button behind the modal).
  const { currentHotkeyLabel } = useHotkeys();

  function openHotkeySettings() {
    pageStore.openSettingsWithSearch('hotkeys');
  }

  // Track window focus state to collapse buttons when unfocused
  let windowFocused = true;

  function handleWindowBlur() {
    windowFocused = false;
    copyLastFade.handleLeave();
    inputChangerFade.handleLeave();
    hotkeyFade.handleLeave();
  }

  function handleWindowFocus() {
    windowFocused = true;
  }

  // Start fade timers on mount
  onMount(() => {
    inputChangerFade.startFade();
    hotkeyFade.startFade();
    window.addEventListener('blur', handleWindowBlur);
    window.addEventListener('focus', handleWindowFocus);
  });

  onDestroy(() => {
    copyLastFade.cleanup();
    copyFeedback.cleanup();
    inputChangerFade.cleanup();
    hotkeyFade.cleanup();
    window.removeEventListener('blur', handleWindowBlur);
    window.removeEventListener('focus', handleWindowFocus);
  });
</script>

<div
  class="fixed bottom-4 left-4 z-50 flex flex-col-reverse gap-3 transition-opacity duration-500 ease-out-quint {$bottomLeftReady
    ? 'opacity-100'
    : 'opacity-0'}"
>
  <!-- Hotkey Button: opens Settings filtered to "hotkeys" -->
  {#if showHotkeyChanger}
    <div class="group/hotkey">
      <button
        on:click={openHotkeySettings}
        on:mouseenter={hotkeyFade.handleEnter}
        on:mouseleave={hotkeyFade.handleLeave}
        class="flex items-center justify-center size-10 {windowFocused
          ? 'group-hover/hotkey:w-[12rem]'
          : ''} bg-zinc-900/90 hover:bg-zinc-800/95 rounded-full border transition-[color,background-color,border-color,width] duration-200 text-sm cursor-pointer shadow-lg shadow-black/30 outline-none focus:outline-none
          {isHistoryHovering
          ? 'border-amber-700/30 text-amber-400/60 hover:border-amber-500/40 hover:text-amber-400'
          : 'border-zinc-700/40 text-zinc-400 hover:border-cyan-500/40 hover:text-cyan-400'}
          {$hotkeyFading && !$hotkeyHovered ? 'opacity-40' : 'opacity-100'}"
        aria-label={t('bottomActions.changeHotkeyAria')}
      >
        <Icon name="keyboard" class="w-4 h-4 flex-shrink-0" strokeWidth={2} />
        <span
          class="{windowFocused
            ? 'w-0 group-hover/hotkey:w-32 group-hover/hotkey:ml-2'
            : 'w-0'} overflow-hidden whitespace-nowrap hotkey-label"
        >
          {$currentHotkeyLabel}
        </span>
      </button>
    </div>
  {/if}

  <!-- Input Changer Button (middle - longest text expansion) -->
  {#if showInputChanger}
    <div class="group/mic">
      <button
        on:click={openRecordingSettings}
        on:mouseenter={inputChangerFade.handleEnter}
        on:mouseleave={inputChangerFade.handleLeave}
        class="flex items-center justify-center size-10 {windowFocused
          ? 'group-hover/mic:w-[17rem]'
          : ''} bg-zinc-900/90 hover:bg-zinc-800/95 rounded-full border transition-[color,background-color,border-color,width] duration-200 text-sm cursor-pointer shadow-lg shadow-black/30 outline-none focus:outline-none
          {isHistoryHovering
          ? 'border-amber-700/30 text-amber-400/60 hover:border-amber-500/40 hover:text-amber-400'
          : 'border-zinc-700/40 text-zinc-400 hover:border-cyan-500/40 hover:text-cyan-400'}
          {$inputChangerFading && !$inputChangerHovered ? 'opacity-40' : 'opacity-100'}"
        aria-label={t('bottomActions.selectMicAria')}
      >
        <Icon name="mic" class="w-4 h-4 flex-shrink-0" strokeWidth={2} />
        <span
          class="w-0 {windowFocused
            ? 'group-hover/mic:w-52 group-hover/mic:ml-2'
            : ''} overflow-hidden whitespace-nowrap">{t('bottomActions.systemDefaultMic')}</span
        >
      </button>
    </div>
  {/if}

  <!-- Copy Last Transcription Button (top of the stack, history amber). Last in
       markup because the stack is flex-col-reverse; up here, hiding it while
       recording doesn't shift the other buttons. -->
  {#if showCopyLastButton && (!isRecording || isOpenMicActive) && !isProcessing && $transcription.currentResult?.text}
    {@const trimmedText = $transcription.currentResult.text.trim()}
    {@const previewText = trimmedText.length > 50 ? trimmedText.slice(0, 50) + '...' : trimmedText}
    <div class="group/copy relative">
      <button
        on:click={copyLastTranscription}
        on:mouseenter={() => {
          copyLastFade.handleEnter();
          isHistoryHovering = true;
          onHistoryHoverChange?.(true);
          requestHistoryPeek(true);
        }}
        on:mouseleave={() => {
          copyLastFade.handleLeave();
          isHistoryHovering = false;
          onHistoryHoverChange?.(false);
          requestHistoryPeek(false);
        }}
        class="flex items-center justify-center size-10 {windowFocused
          ? 'group-hover/copy:w-[8rem]'
          : ''} bg-zinc-900/90 rounded-full border transition-[color,background-color,border-color,width] duration-200 text-sm cursor-pointer shadow-lg shadow-black/30
          {isHistoryHovering || $isCopied
          ? 'border-amber-500/40 text-amber-400 hover:bg-amber-950/30'
          : 'border-amber-700/40 text-amber-400/80 hover:bg-amber-950/30 hover:border-amber-500/40 hover:text-amber-400'}
          {$copyLastFading && !$copyLastHovered ? 'opacity-40' : 'opacity-100'}"
        aria-label={t('bottomActions.copyLastAria')}
      >
        {#if $isCopied}
          <Icon name="check" class="w-4 h-4 text-amber-400 flex-shrink-0" strokeWidth={2} />
          <span
            class="w-0 {windowFocused
              ? 'group-hover/copy:w-14 group-hover/copy:ml-2'
              : ''} overflow-hidden whitespace-nowrap text-amber-400">{t('common.copied')}</span
          >
        {:else}
          <Icon name="copy" class="w-4 h-4 flex-shrink-0" strokeWidth={2} />
          <span
            class="w-0 {windowFocused
              ? 'group-hover/copy:w-[4.5rem] group-hover/copy:ml-2'
              : ''} overflow-hidden whitespace-nowrap">{t('bottomActions.copyLast')}</span
          >
        {/if}
      </button>

      <!-- Tooltip with preview (shows above, no redundant title, solid background) -->
      <div
        class="absolute bottom-full left-0 mb-2 px-3 py-2 bg-zinc-900 rounded-lg border border-amber-700/30 text-xs max-w-[220px] z-[60] opacity-0 pointer-events-none {windowFocused
          ? 'group-hover/copy:opacity-100'
          : ''} transition-opacity duration-100 delay-100 shadow-xl shadow-black/50"
      >
        <p class="text-amber-300/80 italic truncate">"{previewText}"</p>
      </div>
    </div>
  {/if}
</div>

<style>
  /* Hotkey label: macOS modifier glyphs (⌘ ⇧ ⌥) are designed for the system UI
     font, not monospace - render them in the system stack, a touch larger, so they
     look crisp instead of tiny/ugly. */
  .hotkey-label {
    font-family:
      -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif;
    font-size: 0.875rem; /* 14px */
    font-weight: 500;
    letter-spacing: 0.02em;
  }
</style>
