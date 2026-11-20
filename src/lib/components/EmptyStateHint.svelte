<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Empty State Hint
   *
   * Contextual hint messages below the record button.
   * Hides for the rest of the session after the first successful transcription.
   * Nothing is persisted: the in-memory transcription history is the signal,
   * so the hint comes back on every launch.
   */

  import { tick } from 'svelte';
  import { modelStore } from '$lib/stores/modelStore';
  import { transcription } from '$lib/stores/transcription';
  import { hotkeyStore, formatHotkey } from '$lib/stores/hotkeys';
  import { hotkeyAliases } from '$lib/stores/hotkeyAliases';
  import { onboardingStore } from '$lib/stores/onboarding';
  import { demoSession, DEMO_HOTKEY } from '$lib/stores/demo';
  import { resolveEmptyStateHint } from '$lib/utils/emptyStateHint';
  import { fade } from 'svelte/transition';
  import { t } from '$lib/locales';

  interface Props {
    isRecording?: boolean;
    isProcessing?: boolean;
    onOpenDownloadModal?: () => void;
  }

  let { isRecording = false, isProcessing = false, onOpenDownloadModal }: Props = $props();

  // Pass $hotkeyAliases explicitly so renaming a key re-renders this label.
  // The scripted demo advertises the default hotkey no matter what the user bound.
  const hotkey = $derived(
    $demoSession
      ? formatHotkey(DEMO_HOTKEY)
      : formatHotkey($hotkeyStore.currentHotkey, { aliases: $hotkeyAliases })
  );
  const hotkeyReady = $derived($hotkeyStore.initialized);
  const modelExists = $derived($modelStore.modelExists);
  const isPreloading = $derived($modelStore.isPreloading);
  const onboardingComplete = $derived($onboardingStore.completed);
  // History is appended on every completed transcription and never persisted.
  const hasTranscribedThisSession = $derived($transcription.history.length > 0);

  // Programmatic focus on the action CTA when the hint first appears with
  // an actionable handler. Without this the record button is disabled
  // (no model yet) and a keyboard/screen-reader user has no obvious next
  // step. We only auto-focus when an action is actually present and we
  // haven't already done so for this hint instance.
  let actionButton: HTMLButtonElement | null = $state(null);
  let didAutoFocus = false;
  $effect(() => {
    if (actionButton && !didAutoFocus) {
      didAutoFocus = true;
      tick().then(() => actionButton?.focus({ preventScroll: true }));
    }
    if (!actionButton) didAutoFocus = false;
  });

  // Determine which hint to show
  const hint = $derived.by(() => {
    const kind = resolveEmptyStateHint({
      isRecording,
      isProcessing,
      isPreloading,
      onboardingComplete,
      hasTranscribedThisSession,
      modelExists,
      hotkeyReady,
    });

    if (kind === 'none') return null;

    if (kind === 'downloadModel') {
      return {
        text: t('emptyState.downloadModel'),
        action: onOpenDownloadModal
          ? { label: t('emptyState.download'), handler: onOpenDownloadModal }
          : undefined,
      };
    }

    return {
      text: t('emptyState.pressToStart', { hotkey }),
    };
  });
</script>

<div class="hint-slot">
  {#if hint}
    <div class="hint-container" transition:fade={{ duration: 400 }}>
      <p class="hint-text">
        {hint.text}
        {#if hint.action}
          <button bind:this={actionButton} class="hint-action" onclick={hint.action.handler}
            >{hint.action.label}</button
          >
        {/if}
      </p>
    </div>
  {/if}
</div>

<style>
  /* Reserved row under the record button. Fixed height so the group stays
     centered whether or not a hint is showing. */
  .hint-slot {
    height: 1.25rem;
    margin-top: 1.75rem;
    display: flex;
    justify-content: center;
    align-items: center;
    pointer-events: none;
    z-index: 5;
  }

  .hint-container {
    display: flex;
    justify-content: center;
  }

  .hint-text {
    color: #71717a;
    font-size: 0.8125rem;
    white-space: nowrap;
    margin: 0;
    animation: hint-fade-in 0.6s ease-out;
    pointer-events: auto;
  }

  .hint-action {
    background: none;
    border: none;
    color: #a78bfa;
    font-size: 0.8125rem;
    cursor: pointer;
    padding: 0;
    margin-left: 0.25rem;
    text-decoration: underline;
    text-decoration-color: rgba(167, 139, 250, 0.3);
    text-underline-offset: 2px;
    transition: color 0.15s ease;
  }

  .hint-action:hover {
    color: #c4b5fd;
  }

  @keyframes hint-fade-in {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
