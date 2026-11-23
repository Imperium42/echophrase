<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import HintTooltip from './settings/primitives/HintTooltip.svelte';
  /**
   * First-Run Onboarding Overlay
   *
   * Lightweight welcome flow with spotlight highlights.
   * Stays on the main screen - not a separate wizard.
   *
   * Note: as of the ort 2.0.0-rc.12 upgrade, GPU acceleration is auto-detected
   * by ort's `with_auto_device(MaxPerformance)` at session creation time, so
   * there is no GPU setup step.
   */

  import { onMount, onDestroy } from 'svelte';
  import { onboardingStore } from '$lib/stores/onboarding';
  import { startPageLoadAnimation } from '$lib/composables/usePageLoadAnimation';
  import { modelStore } from '$lib/stores/modelStore';
  import { hotkeyStore, formatHotkey } from '$lib/stores/hotkeys';
  import { hotkeyAliases } from '$lib/stores/hotkeyAliases';
  import { fade } from 'svelte/transition';
  import { Icon } from '$lib/icons';
  import { isLinux } from '$lib/utils/platform';
  import { t } from '$lib/locales';
  import { getModelById } from '$lib/types/models';
  import LanguageSelector from './LanguageSelector.svelte';
  import { useTauriSettings } from '$lib/composables/useTauriSettings';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { docsUrl } from '$lib/utils/docsUrl';
  import {
    ensureMicPermission,
    refreshMicPermissionStatus,
    openMicSystemSettings,
    requiresExplicitMicPermission,
    micPermissionStatus,
  } from '$lib/composables/useMicPermission';
  import {
    accessibilityGranted,
    currentAccessibilityGranted,
    openAccessibilitySystemSettings,
    refreshAccessibilityPermission,
    requestPastePermission,
  } from '$lib/composables/useAccessibilityPermission';

  interface Props {
    onOpenDownloadModal?: () => void;
    isModalOpen?: boolean;
    isDownloadActive?: boolean;
  }

  let { onOpenDownloadModal, isModalOpen = false, isDownloadActive = false }: Props = $props();

  const { updateSetting } = useTauriSettings();

  // Defer overlay render by one frame so WebKitGTK can establish title bar hit zones
  let overlayReady = $state(false);
  const needsHotkeyWarning = isLinux();
  const needsMicPermission = requiresExplicitMicPermission();

  // Step layout: welcome (0), [mic permission], download (1+offset), ready (2+offset), [hotkey warning]
  const micPermStepIndex = $derived(needsMicPermission ? 1 : -1);
  const downloadStepIndex = $derived(1 + (needsMicPermission ? 1 : 0));
  const readyStepIndex = $derived(2 + (needsMicPermission ? 1 : 0));
  const hotkeyWarningIndex = $derived(needsHotkeyWarning ? readyStepIndex + 1 : -1);
  const TOTAL_STEPS = $derived(3 + (needsMicPermission ? 1 : 0) + (needsHotkeyWarning ? 1 : 0));

  const permStatus = $derived($micPermissionStatus);

  async function handleGrantMic() {
    const status = await ensureMicPermission();
    if (status === 'denied' || status === 'restricted') {
      await openMicSystemSettings();
    }
  }

  /** Set once the user picks the clipboard route instead of Accessibility. */
  let choseClipboard = $state(false);

  /** Shown when the permission is on in System Settings but not visible to us yet. */
  let axNeedsRestart = $state(false);

  /** Both permissions settled, so the step has nothing left to ask for. */
  const allPermissionsGranted = $derived(permStatus === 'authorized' && $accessibilityGranted);

  /**
   * "Grant access" for keystroke posting.
   *
   * Asks the OS directly first (`CGRequestPostEventAccess`), which both shows the
   * system dialog and adds Echophrase to the allow-list, so the common case takes
   * one click. Only if that comes back denied do we send the user to the settings
   * pane to flip the toggle by hand.
   *
   * We poll afterwards, but cannot rely on it: CoreGraphics caches a negative
   * answer per-process, so a grant made while the app runs may not be visible
   * until restart. Hence the restart hint rather than an indefinite spinner.
   */
  let axPoll: ReturnType<typeof setInterval> | null = null;
  let axPollDeadline: ReturnType<typeof setTimeout> | null = null;

  function clearAccessibilityPoll() {
    if (axPoll) clearInterval(axPoll);
    if (axPollDeadline) clearTimeout(axPollDeadline);
    axPoll = null;
    axPollDeadline = null;
  }

  async function handleGrantAccessibility() {
    axNeedsRestart = false;
    if (await requestPastePermission()) return;

    await openAccessibilitySystemSettings();
    clearAccessibilityPoll();
    axPoll = setInterval(async () => {
      if (await refreshAccessibilityPermission()) {
        clearAccessibilityPoll();
        axNeedsRestart = false;
      }
    }, 1500);
    // Give the user time to find the toggle, then tell them what to do rather
    // than polling forever against a cached answer.
    axPollDeadline = setTimeout(() => {
      clearAccessibilityPoll();
      if (!currentAccessibilityGranted()) axNeedsRestart = true;
    }, 20_000);
  }

  onDestroy(clearAccessibilityPoll);

  /**
   * "Copy to clipboard, instead" - switches output away from paste-at-cursor so
   * no permission is needed at all.
   */
  function handleUseClipboard() {
    updateSetting('output_to', ['copy_to_clipboard']);
    choseClipboard = true;
  }

  const step = $derived($onboardingStore.currentStep);
  // Pass $hotkeyAliases explicitly so renaming a key re-renders this label.
  const hotkey = $derived(formatHotkey($hotkeyStore.currentHotkey, { aliases: $hotkeyAliases }));

  onMount(() => {
    requestAnimationFrame(() => {
      overlayReady = true;
    });
    if (needsMicPermission) {
      void refreshMicPermissionStatus();
      void refreshAccessibilityPermission();
    }
  });

  async function next() {
    // When leaving the hotkey warning step on Linux, set open mic as default
    if (step === hotkeyWarningIndex && needsHotkeyWarning) {
      updateSetting('recording_mode', 'open_mic');
    }

    if (step >= TOTAL_STEPS - 1) {
      // Replay the main screen's entrance animation as the overlay fades out -
      // it already ran once behind the overlay during onMount, so without this
      // the user just sees the finished, static UI the instant onboarding closes.
      startPageLoadAnimation();
      onboardingStore.complete();
    } else {
      onboardingStore.nextStep();
    }
  }

  let showSkipConfirm = $state(false);

  async function skip() {
    // Same reveal as finishing normally - skip also lands the user on the main
    // screen for the first time.
    startPageLoadAnimation();
    onboardingStore.skip();
  }

  function confirmSkip() {
    showSkipConfirm = true;
  }

  function handleDownload() {
    onOpenDownloadModal?.();
    void next();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (isModalOpen || showSkipConfirm) return;
    if (e.key === 'Escape' && step > 0) confirmSkip();

    // Space/Enter belongs to whatever control has focus, not to "advance". Without
    // this, pressing Space on a focused checkbox or button toggled it AND stepped
    // past the step in one keypress.
    const target = e.target as HTMLElement | null;
    const tag = target?.tagName;
    if (tag === 'INPUT' || tag === 'BUTTON' || tag === 'A' || tag === 'TEXTAREA') return;

    if (e.key === 'Enter' || e.key === ' ') void next();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if overlayReady}
  <div class="onboarding-overlay" transition:fade={{ duration: 300 }}>
    <div class="onboarding-backdrop"></div>

    <div class="onboarding-card">
      <!-- Language selector on welcome step -->
      {#if step === 0}
        <div class="onboarding-lang">
          <LanguageSelector />
        </div>
      {/if}

      <!-- Step indicators -->
      <div class="step-dots">
        {#each Array(TOTAL_STEPS) as _, i}
          <div class="dot" class:active={i === step} class:completed={i < step}></div>
        {/each}
      </div>

      {#key step}
        <div class="step-content">
          {#if step === 0}
            <div class="step-icon">
              <Icon name="mic" class="w-8 h-8 text-cyan-400" strokeWidth={1.5} />
            </div>
            <h3 class="step-title">{t('onboarding.welcome')}</h3>
            <p class="step-text">
              {t('onboarding.welcomeDescription')}
              <br />
              <br />
              {t('onboarding.welcomeNoTelemetry')}
            </p>
          {:else if step === micPermStepIndex}
            <div class="step-icon">
              <Icon name="lock" class="w-8 h-8 text-cyan-400" strokeWidth={1.5} />
            </div>
            <h3 class="step-title">{t('onboarding.permissionsTitle')}</h3>

            <!-- 1. Microphone: non-negotiable, the app cannot hear you without it -->
            <div class="perm-item">
              <div class="perm-head">
                <span class="perm-num">1</span>
                <div class="perm-copy">
                  <span class="perm-name">{t('onboarding.micPermissionTitle')}</span>
                  <span class="perm-why">{t('onboarding.micPermissionDescription')}</span>
                </div>
              </div>
              <div class="mic-perm-row">
                {#if permStatus === 'authorized'}
                  <span class="mic-perm-badge mic-perm-ok">
                    <Icon name="check" size={14} strokeWidth={2.5} />
                    {t('onboarding.micPermissionGranted')}
                  </span>
                {:else if permStatus === 'denied' || permStatus === 'restricted'}
                  <span class="mic-perm-badge mic-perm-bad"
                    >{t('onboarding.micPermissionDenied')}</span
                  >
                  <button class="action-btn" onclick={openMicSystemSettings}>
                    {t('onboarding.micPermissionOpenSettings')}
                  </button>
                {:else}
                  <button class="action-btn" onclick={handleGrantMic}>
                    {t('onboarding.micPermissionGrant')}
                  </button>
                {/if}
              </div>
            </div>

            <!-- 2. Accessibility: optional. The clipboard route needs no permission
               at all, so it gets equal billing rather than being buried. -->
            <div class="perm-item">
              <div class="perm-head">
                <span class="perm-num">2</span>
                <div class="perm-copy">
                  <span class="perm-name">{t('onboarding.axPermissionTitle')}</span>
                  <span class="perm-why">{t('onboarding.axPermissionDescription')}</span>
                </div>
              </div>
              <div class="mic-perm-row">
                {#if $accessibilityGranted}
                  <span class="mic-perm-badge mic-perm-ok">
                    <Icon name="check" size={14} strokeWidth={2.5} />
                    {t('onboarding.axPermissionGranted')}
                  </span>
                {:else if choseClipboard}
                  <span class="mic-perm-badge mic-perm-ok">
                    <Icon name="check" size={14} strokeWidth={2.5} />
                    {t('onboarding.axChoseClipboard')}
                  </span>
                {:else}
                  <button class="action-btn" onclick={handleGrantAccessibility}>
                    {t('onboarding.axPermissionGrant')}
                  </button>
                  <button class="action-btn action-btn-alt" onclick={handleUseClipboard}>
                    {t('onboarding.axUseClipboard')}
                  </button>
                {/if}
              </div>
              {#if choseClipboard}
                <p class="perm-note">{t('onboarding.axChoseClipboardNote')}</p>
              {:else if axNeedsRestart}
                <p class="perm-note">{t('onboarding.axNeedsRestart')}</p>
              {/if}
            </div>

            <!-- Nothing left to ask for: offer a way to go review what was granted. -->
            {#if allPermissionsGranted}
              <div class="perm-manage">
                <span class="perm-manage-label">{t('onboarding.managePermissions')}</span>
                <button class="perm-manage-link" onclick={openMicSystemSettings} type="button">
                  {t('onboarding.managePermissionsMic')}
                </button>
                <span class="perm-manage-sep" aria-hidden="true">|</span>
                <button
                  class="perm-manage-link"
                  onclick={openAccessibilitySystemSettings}
                  type="button"
                >
                  {t('onboarding.managePermissionsAx')}
                </button>
              </div>
            {/if}

            <!-- launchUrl, not a bare anchor: target=_blank does nothing inside the Tauri webview.
               Reuses the same book-icon docs-link pattern as CustomModelSection/McpServerSection.
               Deep link to the public "Permissions" doc page - ties the abbreviated in-app copy
               to the full explainer (why Accessibility only needs the narrow post-event
               capability, the clipboard fallback, the restart quirk). -->
            <button
              class="perm-docs-link"
              type="button"
              onclick={() => launchUrl(docsUrl('/troubleshooting/permissions'))}
            >
              <Icon name="book" size={12} />
              {t('onboarding.permissionsDocsLink')}
            </button>
          {:else if step === downloadStepIndex}
            <div class="step-icon">
              <Icon name="monitor" class="w-8 h-8 text-purple-400" strokeWidth={1.5} />
            </div>
            <h3 class="step-title">{t('onboarding.downloadModel')}</h3>
            <p class="step-text">
              {t('onboarding.downloadModelDescription')}
            </p>
            {#if !$modelStore.modelExists}
              {#if isDownloadActive}
                <p class="step-text" style="color: var(--cyan-400, #22d3ee)">
                  {t('onboarding.downloadingInProgress')}
                </p>
              {:else}
                <button class="action-btn" onclick={handleDownload}>
                  {t('onboarding.downloadModelButton')}
                </button>
              {/if}
            {:else}
              {@const model = getModelById($modelStore.selectedModel)}
              <div class="model-indicator">
                <div class="model-indicator-row">
                  <Icon name="check" size={14} class="shrink-0 text-[#a3e635]" strokeWidth={2.5} />
                  <span class="model-name model-valid"
                    >{model?.name ?? $modelStore.selectedModel}</span
                  >
                  {#if model?.isRecommended}
                    <span class="model-recommended">{t('onboarding.recommended')}</span>
                  {/if}
                </div>
                <div class="model-indicator-row model-indicator-actions">
                  <button class="model-change-btn" onclick={() => onOpenDownloadModal?.()}
                    >{t('onboarding.changeModel')}</button
                  >
                  <HintTooltip hint={t('onboarding.modelHint')} />
                </div>
              </div>
            {/if}
          {:else if step === readyStepIndex}
            <div class="step-icon">
              <Icon name="mouse-click" class="w-8 h-8 text-emerald-400" strokeWidth={1.5} />
            </div>
            <h3 class="step-title">
              {needsHotkeyWarning ? t('onboarding.almostReady') : t('onboarding.readyToGo')}
            </h3>
            <ul class="step-text step-list">
              <li>{t('onboarding.readyStepClick').replace('{hotkey}', hotkey)}</li>
              {#if !needsHotkeyWarning}
                <!-- Linux switches to open mic here, where hold-to-talk doesn't apply. -->
                <li>{t('onboarding.readyStepHold').replace('{hotkey}', hotkey)}</li>
              {/if}
            </ul>
            <p class="step-text-sub">{t('onboarding.readyStepOutput')}</p>
          {:else if step === hotkeyWarningIndex}
            <div class="step-icon">
              <Icon name="mic" class="w-8 h-8 text-violet-400" strokeWidth={1.5} />
            </div>
            <h3 class="step-title">{t('onboarding.openMicMode')}</h3>
            <p class="step-text">
              {t('onboarding.openMicModeDescription')}
            </p>
            <p class="step-text-sub">
              {t('onboarding.openMicModeSub')}
            </p>
          {/if}
        </div>
      {/key}

      <!-- Navigation -->
      <div class="step-nav">
        <button class="skip-btn" onclick={skip}>{t('onboarding.skip')}</button>
        <div class="step-nav-right">
          {#if step > 0}
            <button class="back-btn" onclick={() => onboardingStore.prevStep()} aria-label="Back">
              <Icon name="arrow-left" class="w-3.5 h-3.5" />
            </button>
          {/if}
          {#if step === hotkeyWarningIndex}
            <button class="next-btn" onclick={next}>{t('onboarding.iUnderstand')}</button>
          {:else if step >= TOTAL_STEPS - 1}
            <button class="next-btn" onclick={next}>{t('onboarding.letsGo')}</button>
          {:else}
            <button class="arrow-btn" onclick={next} aria-label="Next">
              <Icon name="arrow-right" class="w-4 h-4" />
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

{#if showSkipConfirm}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="skip-confirm-backdrop"
    onclick={() => (showSkipConfirm = false)}
    onkeydown={e => {
      if (e.key === 'Escape') showSkipConfirm = false;
    }}
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="skip-confirm-modal" onclick={e => e.stopPropagation()} onkeydown={() => {}}>
      <p class="skip-confirm-text">{t('onboarding.skipConfirm')}</p>
      <div class="skip-confirm-actions">
        <button class="skip-confirm-cancel" onclick={() => (showSkipConfirm = false)}
          >{t('onboarding.skipNo')}</button
        >
        <button class="skip-confirm-yes" onclick={skip}>{t('onboarding.skipYes')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .skip-confirm-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 150;
  }

  .skip-confirm-modal {
    background: #18181b;
    border: 1px solid #3f3f46;
    border-radius: 0.75rem;
    padding: 1.25rem 1.5rem;
    max-width: 18rem;
    text-align: center;
  }

  .skip-confirm-text {
    font-size: 0.875rem;
    color: #d4d4d8;
    margin: 0 0 1rem;
  }

  .skip-confirm-actions {
    display: flex;
    gap: 0.75rem;
    justify-content: center;
  }

  .skip-confirm-cancel {
    font-size: 0.8125rem;
    color: #a1a1aa;
    background: none;
    border: 1px solid #3f3f46;
    border-radius: 0.375rem;
    padding: 0.375rem 0.875rem;
    cursor: pointer;
  }

  .skip-confirm-cancel:hover {
    background: rgba(255, 255, 255, 0.04);
    color: #d4d4d8;
  }

  .skip-confirm-yes {
    font-size: 0.8125rem;
    color: #fbbf24;
    background: none;
    border: 1px solid rgba(251, 191, 36, 0.3);
    border-radius: 0.375rem;
    padding: 0.375rem 0.875rem;
    cursor: pointer;
  }

  .skip-confirm-yes:hover {
    background: rgba(251, 191, 36, 0.1);
  }

  .onboarding-overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .onboarding-backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(4px);
  }

  .onboarding-card {
    position: relative;
    background: #18181b;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 1rem;
    padding: 2rem;
    max-width: 24rem;
    width: 90%;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
  }

  .onboarding-lang {
    position: absolute;
    top: 1rem;
    right: 1rem;
    z-index: 10;
  }

  .step-dots {
    display: flex;
    justify-content: center;
    gap: 0.5rem;
    margin-bottom: 1.5rem;
  }

  .dot {
    width: 0.375rem;
    height: 0.375rem;
    border-radius: 9999px;
    background: #3f3f46;
    transition: all 0.3s ease;
  }

  .dot.active {
    background: #22d3ee;
    width: 1.25rem;
  }

  .dot.completed {
    background: #06b6d4;
  }

  .step-content {
    text-align: center;
    min-height: 13rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
  }

  .step-icon {
    margin-bottom: 1rem;
    opacity: 0.9;
  }

  .step-title {
    font-size: 1.125rem;
    font-weight: 600;
    color: #e4e4e7;
    margin: 0 0 0.5rem;
    letter-spacing: -0.01em;
  }

  .step-list {
    padding-left: 1.25rem;
    list-style: disc;
    text-align: left;
  }

  .step-text {
    text-wrap: balance;
    font-size: 0.8125rem;
    color: #a1a1aa;
    line-height: 1.5;
    margin: 0;
    max-width: 20rem;
  }

  .model-indicator {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    margin-top: 0.75rem;
  }

  .model-indicator-row {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    white-space: nowrap;
  }

  .model-indicator-actions {
    gap: 0.375rem;
  }

  .model-name {
    font-size: 0.8125rem;
    color: #d4d4d8;
  }

  .model-name.model-valid {
    color: #a3e635;
  }

  .model-recommended {
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #4ade80;
    background: rgba(74, 222, 128, 0.1);
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
  }

  .model-change-btn {
    font-size: 0.75rem;
    color: #a78bfa;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .model-change-btn:hover {
    color: #c4b5fd;
  }

  .mic-perm-row {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }

  .perm-item {
    width: 100%;
    padding: 0.75rem 0.875rem;
    margin-top: 0.75rem;
    background: rgba(39, 39, 42, 0.45);
    border: 1px solid rgba(63, 63, 70, 0.5);
    border-radius: 0.625rem;
    text-align: left;
  }

  .perm-head {
    display: flex;
    align-items: flex-start;
    gap: 0.625rem;
  }

  .perm-num {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 1.25rem;
    height: 1.25rem;
    border-radius: 9999px;
    background: rgba(34, 211, 238, 0.15);
    color: #22d3ee;
    font-size: 0.6875rem;
    font-weight: 700;
  }

  .perm-copy {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .perm-name {
    font-size: 0.8125rem;
    font-weight: 600;
    color: #e4e4e7;
  }

  .perm-why {
    font-size: 0.75rem;
    color: #a1a1aa;
    line-height: 1.45;
  }

  .perm-manage {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-wrap: wrap;
    gap: 0.375rem;
    margin-top: 0.875rem;
    font-size: 0.75rem;
  }

  .perm-manage-label {
    color: #71717a;
  }

  .perm-manage-sep {
    color: #3f3f46;
  }

  .perm-manage-link {
    padding: 0;
    background: none;
    border: none;
    font-size: 0.75rem;
    font-weight: 500;
    color: #a1a1aa;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .perm-manage-link:hover {
    color: #22d3ee;
  }

  .perm-manage-link:focus-visible {
    outline: 2px solid #22d3ee;
    outline-offset: 2px;
    border-radius: 0.25rem;
  }

  .perm-docs-link {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.3rem;
    margin: 0.5rem auto 0;
    padding: 0;
    background: none;
    border: none;
    font-size: 0.75rem;
    color: #71717a;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .perm-docs-link:hover {
    color: #22d3ee;
  }

  .perm-docs-link:focus-visible {
    outline: 2px solid #22d3ee;
    outline-offset: 2px;
    border-radius: 0.25rem;
  }

  .perm-note {
    font-size: 0.6875rem;
    color: #71717a;
    line-height: 1.5;
    margin: 0.5rem 0 0;
    font-style: italic;
  }

  /* The no-permission alternative sits beside "Grant access" as a peer choice,
     visually quieter but not hidden. */
  .action-btn-alt {
    margin-top: 0;
    background: transparent;
    border: 1px solid rgba(63, 63, 70, 0.7);
    color: #a1a1aa;
  }

  .action-btn-alt:hover {
    background: rgba(63, 63, 70, 0.3);
    color: #d4d4d8;
  }

  .mic-perm-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.25rem 0.625rem;
    border-radius: 9999px;
    border: 1px solid transparent;
  }

  .mic-perm-ok {
    color: #a3e635;
    background: rgba(163, 230, 53, 0.08);
    border-color: rgba(163, 230, 53, 0.25);
  }

  .mic-perm-bad {
    color: #fca5a5;
    background: rgba(239, 68, 68, 0.1);
    border-color: rgba(239, 68, 68, 0.3);
  }

  .step-text-sub {
    font-size: 0.6875rem;
    color: #71717a;
    line-height: 1.5;
    margin: 0.5rem 0 0;
    font-style: italic;
  }

  .action-btn {
    margin-top: 1rem;
    padding: 0.5rem 1.25rem;
    background: linear-gradient(135deg, #7c3aed, #6d28d9);
    color: white;
    border: none;
    border-radius: 0.5rem;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition: opacity 0.15s ease;
  }

  .action-btn:hover {
    opacity: 0.9;
  }

  .step-nav {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 1.5rem;
    padding-top: 1rem;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
  }

  .skip-btn {
    background: none;
    border: none;
    color: #52525b;
    font-size: 0.75rem;
    cursor: pointer;
    padding: 0.375rem 0.75rem;
    border-radius: 0.375rem;
    transition: color 0.15s ease;
  }

  .skip-btn:hover {
    color: #71717a;
  }

  .step-nav-right {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .back-btn {
    width: 2rem;
    height: 2rem;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 0.375rem;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .back-btn:hover {
    background: rgba(255, 255, 255, 0.04);
    color: #a1a1aa;
  }

  .arrow-btn {
    width: 2.5rem;
    height: 2.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.5rem;
    color: #e4e4e7;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .arrow-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .next-btn {
    padding: 0.5rem 1.25rem;
    height: 2.25rem;
    display: flex;
    align-items: center;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.5rem;
    color: #e4e4e7;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .next-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }
</style>
