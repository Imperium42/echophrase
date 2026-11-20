<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { t } from '$lib/locales';
  import ModalWindow from './ModalWindow.svelte';
  import { Icon } from '$lib/icons';
  import {
    accessibilityGranted,
    currentAccessibilityGranted,
    openAccessibilitySystemSettings,
    refreshAccessibilityPermission,
    requestPastePermission,
    showAccessibilityModal,
  } from '$lib/composables/useAccessibilityPermission';
  import { onDestroy } from 'svelte';

  /**
   * Accessibility Permission Modal (macOS)
   *
   * Shown when a "paste at cursor" attempt is blocked because the app is not in
   * the Accessibility allow-list. macOS's own prompt says only that Echophrase
   * "would like to control this computer", which is alarming and explains
   * nothing - this modal supplies the missing WHY, and offers the clipboard as
   * a no-permission alternative so the request never feels mandatory.
   */

  /** Polls trust while the user is away in System Settings, so the modal self-closes. */
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  function stopPolling() {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  /** True once we know the toggle needs a relaunch to take effect. */
  let needsRestart = $state(false);

  async function handleOpenSettings() {
    needsRestart = false;
    // Ask the OS for the narrow post-event permission first: one click in the
    // common case, and it adds Echophrase to the list for us.
    if (await requestPastePermission()) {
      showAccessibilityModal.set(false);
      return;
    }
    await openAccessibilitySystemSettings();
    stopPolling();
    pollTimer = setInterval(async () => {
      if (await refreshAccessibilityPermission()) {
        stopPolling();
        showAccessibilityModal.set(false);
      }
    }, 1500);
    // CoreGraphics caches a negative answer per-process, so polling can never
    // see a mid-session grant. Say so instead of spinning indefinitely.
    setTimeout(() => {
      stopPolling();
      if (!currentAccessibilityGranted()) needsRestart = true;
    }, 20_000);
  }

  function handleClose() {
    stopPolling();
    showAccessibilityModal.set(false);
  }

  onDestroy(stopPolling);

  // Stop polling if trust arrives via any other route (e.g. an app restart).
  $effect(() => {
    if ($accessibilityGranted) stopPolling();
  });
</script>

<ModalWindow isOpen={$showAccessibilityModal} maxWidth="max-w-md" onClose={handleClose}>
  <div class="modal-content">
    <div class="icon-container">
      <Icon name="keyboard" size={32} color="rgb(34, 211, 238)" />
    </div>

    <h2 class="title">{t('accessibilityPermission.title')}</h2>
    <p class="subtitle">{t('accessibilityPermission.subtitle')}</p>

    <div class="info-box">
      <p class="info-text">{t('accessibilityPermission.why')}</p>
    </div>

    <ol class="steps">
      <li>
        {t('accessibilityPermission.step1Prefix')}
        <button class="link" onclick={handleOpenSettings} type="button">
          {t('accessibilityPermission.step1Link')}
        </button>
      </li>
      <li>{t('accessibilityPermission.step2')}</li>
      <li>{t('accessibilityPermission.step3')}</li>
    </ol>

    <div class="actions">
      <button class="primary-button" onclick={handleOpenSettings} type="button">
        <Icon name="external-link" size={16} class="shrink-0" />
        <span>{t('accessibilityPermission.openSettings')}</span>
      </button>
      <button class="secondary-button" onclick={handleClose} type="button">
        {t('accessibilityPermission.notNow')}
      </button>
    </div>

    {#if needsRestart}
      <p class="privacy-note">{t('accessibilityPermission.needsRestart')}</p>
    {/if}

    <p class="privacy-note">{t('accessibilityPermission.clipboardFallback')}</p>
  </div>
</ModalWindow>

<style>
  .modal-content {
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    text-align: center;
  }

  .icon-container {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 4rem;
    height: 4rem;
    background: linear-gradient(135deg, rgba(34, 211, 238, 0.2) 0%, rgba(168, 85, 247, 0.2) 100%);
    border-radius: 50%;
  }

  .title {
    font-size: 1.25rem;
    font-weight: 700;
    white-space: nowrap;
    color: rgb(244, 244, 245);
    margin: 0;
  }

  .subtitle {
    font-size: 0.875rem;
    color: rgb(161, 161, 170);
    margin: 0;
    white-space: nowrap;
  }

  .info-box {
    width: 100%;
    padding: 1rem;
    background: rgba(39, 39, 42, 0.5);
    border-radius: 0.75rem;
    border: 1px solid rgba(63, 63, 70, 0.5);
    text-align: left;
  }

  .info-text {
    font-size: 0.875rem;
    color: rgb(212, 212, 216);
    margin: 0;
  }

  .steps {
    width: 100%;
    margin: 0;
    padding-left: 1.25rem;
    list-style: decimal;
    text-align: left;
    font-size: 0.8125rem;
    color: rgb(161, 161, 170);
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .link {
    display: inline;
    padding: 0;
    background: none;
    border: none;
    font: inherit;
    color: rgb(34, 211, 238);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.5rem;
  }

  .primary-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 0.5rem 1.25rem;
    background: linear-gradient(135deg, rgb(34, 211, 238) 0%, rgb(168, 85, 247) 100%);
    border: none;
    border-radius: 0.75rem;
    font-size: 0.875rem;
    font-weight: 600;
    color: rgb(24, 24, 27);
    cursor: pointer;
    transition:
      opacity 0.2s ease,
      transform 0.1s ease;
  }

  .primary-button:hover {
    opacity: 0.9;
  }

  .primary-button:active {
    transform: scale(0.98);
  }

  .secondary-button {
    padding: 0.25rem 0.75rem;
    background: transparent;
    border: none;
    border-radius: 0.5rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: rgb(161, 161, 170);
    cursor: pointer;
    transition:
      background 0.2s ease,
      color 0.2s ease,
      border-color 0.2s ease;
  }

  .secondary-button:hover {
    background: rgba(63, 63, 70, 0.3);
    color: rgb(212, 212, 216);
  }

  .privacy-note {
    font-size: 0.75rem;
    color: rgb(113, 113, 122);
    margin: 0;
    font-style: italic;
  }

  .primary-button:focus-visible,
  .secondary-button:focus-visible,
  .link:focus-visible {
    outline: 2px solid rgb(34, 211, 238);
    outline-offset: 2px;
  }
</style>
