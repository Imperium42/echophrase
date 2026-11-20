<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { t } from '$lib/locales';
  import ModalWindow from './ModalWindow.svelte';
  import { Icon } from '$lib/icons';

  /**
   * CUDA Required Modal
   *
   * Blocking modal shown when GPU is detected but CUDA DLLs are missing.
   * User must either download DLLs or close the app - no escape.
   */

  export let isOpen = false;

  /** Download progress (0-100) */
  export let downloadProgress = 0;

  /** Current download status */
  export let downloadStatus: 'idle' | 'downloading' | 'extracting' | 'complete' | 'error' = 'idle';

  /** Error message if download failed */
  export let errorMessage: string | null = null;

  /** Download size display (e.g., "~200MB") */
  export let downloadSize = '~200MB';

  const dispatch = createEventDispatcher<{
    download: void;
    close: void;
  }>();

  function handleDownload() {
    dispatch('download');
  }

  function handleCloseApp() {
    dispatch('close');
  }

  // Format progress for display
  $: progressDisplay = Math.round(downloadProgress);

  // Status messages
  $: statusMessage = {
    idle: t('cuda.statusGpuDetected'),
    downloading: t('cuda.statusDownloading', { progress: String(progressDisplay) }),
    extracting: t('cuda.statusExtracting'),
    complete: t('cuda.statusComplete'),
    error: errorMessage || t('cuda.statusFailed'),
  }[downloadStatus];

  // Button states
  $: isDownloading = downloadStatus === 'downloading' || downloadStatus === 'extracting';
  $: isComplete = downloadStatus === 'complete';
  $: isError = downloadStatus === 'error';
</script>

<ModalWindow {isOpen} maxWidth="max-w-md" hideCloseButton>
  <div class="modal-content">
    <!-- GPU Icon -->
    <div class="icon-container">
      <Icon name="cpu" size={32} color="rgb(34, 211, 238)" />
    </div>

    <!-- Title -->
    <h2 class="title">{t('cuda.title')}</h2>
    <p class="subtitle">{t('cuda.downloadSize', { size: downloadSize })}</p>

    <!-- Status message -->
    <p class="status-message" class:error={isError} class:success={isComplete}>
      {statusMessage}
    </p>

    <!-- Progress bar (visible during download) -->
    {#if isDownloading}
      <div class="progress-container">
        <div class="progress-bar">
          <div class="progress-fill" style="width: {downloadProgress}%"></div>
        </div>
        <span class="progress-text">{progressDisplay}%</span>
      </div>
    {/if}

    <!-- Info box -->
    {#if downloadStatus === 'idle'}
      <div class="info-box">
        <p class="info-text">
          <strong>{t('cuda.gpuDetected')}</strong>
          {t('cuda.downloadCuda')}
        </p>
        <p class="info-text secondary">
          {t('cuda.withoutCuda')}
        </p>
      </div>
    {/if}

    <!-- Action buttons -->
    <div class="actions">
      {#if isComplete}
        <button class="primary-button success" on:click={handleCloseApp} type="button">
          <Icon name="check" size={20} class="shrink-0" />
          <span>{t('cuda.continue')}</span>
        </button>
      {:else if isError}
        <button class="primary-button" on:click={handleDownload} type="button">
          <Icon name="refresh" size={20} class="shrink-0" />
          <span>{t('cuda.retryDownload')}</span>
        </button>
      {:else}
        <button
          class="primary-button"
          on:click={handleDownload}
          disabled={isDownloading}
          type="button"
        >
          {#if isDownloading}
            <div class="spinner"></div>
            <span>{t('cuda.downloading')}</span>
          {:else}
            <Icon name="download" size={20} class="shrink-0" />
            <span>{t('cuda.downloadCudaLibraries')}</span>
          {/if}
        </button>
      {/if}

      {#if !isComplete && !isDownloading}
        <button class="secondary-button danger" on:click={handleCloseApp} type="button">
          {t('cuda.closeApp')}
        </button>
      {/if}
    </div>

    <!-- Note about privacy -->
    <p class="privacy-note">
      {t('cuda.localProcessing')}
    </p>
  </div>
</ModalWindow>

<style>
  .modal-content {
    padding: 2rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
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
    font-size: 1.5rem;
    font-weight: 700;
    color: rgb(244, 244, 245);
    margin: 0;
  }

  .subtitle {
    font-size: 0.875rem;
    color: rgb(161, 161, 170);
    margin: 0;
  }

  .status-message {
    font-size: 0.9375rem;
    color: rgb(212, 212, 216);
    margin: 0.5rem 0;
  }

  .status-message.error {
    color: rgb(239, 68, 68);
  }

  .status-message.success {
    color: rgb(34, 197, 94);
  }

  .progress-container {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .progress-bar {
    flex: 1;
    height: 0.5rem;
    background: rgba(39, 39, 42, 0.8);
    border-radius: 0.25rem;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, rgb(34, 211, 238) 0%, rgb(168, 85, 247) 100%);
    border-radius: 0.25rem;
    transition: width 0.3s ease;
  }

  .progress-text {
    font-size: 0.875rem;
    color: rgb(161, 161, 170);
    min-width: 3rem;
    text-align: right;
  }

  .info-box {
    width: 100%;
    padding: 1rem;
    background: rgba(39, 39, 42, 0.5);
    border-radius: 0.75rem;
    border: 1px solid rgba(63, 63, 70, 0.5);
  }

  .info-text {
    font-size: 0.875rem;
    color: rgb(212, 212, 216);
    margin: 0;
  }

  .info-text.secondary {
    color: rgb(113, 113, 122);
    margin-top: 0.5rem;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    width: 100%;
    margin-top: 0.5rem;
  }

  .primary-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    width: 100%;
    padding: 0.875rem 1.5rem;
    background: linear-gradient(135deg, rgb(34, 211, 238) 0%, rgb(168, 85, 247) 100%);
    border: none;
    border-radius: 0.75rem;
    font-size: 1rem;
    font-weight: 600;
    color: rgb(24, 24, 27);
    cursor: pointer;
    transition:
      opacity 0.2s ease,
      transform 0.1s ease;
  }

  .primary-button:hover:not(:disabled) {
    opacity: 0.9;
  }

  .primary-button:active:not(:disabled) {
    transform: scale(0.98);
  }

  .primary-button:disabled {
    opacity: 0.7;
    cursor: not-allowed;
  }

  .primary-button.success {
    background: linear-gradient(135deg, rgb(34, 197, 94) 0%, rgb(22, 163, 74) 100%);
  }

  .secondary-button {
    width: 100%;
    padding: 0.75rem 1.5rem;
    background: transparent;
    border: 1px solid rgba(63, 63, 70, 0.5);
    border-radius: 0.75rem;
    font-size: 0.875rem;
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
    border-color: rgba(63, 63, 70, 0.8);
  }

  .secondary-button.danger {
    border-color: rgba(239, 68, 68, 0.3);
    color: rgb(239, 68, 68);
  }

  .secondary-button.danger:hover {
    background: rgba(239, 68, 68, 0.1);
    border-color: rgba(239, 68, 68, 0.5);
  }

  .spinner {
    width: 1.25rem;
    height: 1.25rem;
    border: 2px solid rgba(24, 24, 27, 0.3);
    border-top-color: rgb(24, 24, 27);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .privacy-note {
    font-size: 0.75rem;
    color: rgb(113, 113, 122);
    margin: 0.5rem 0 0 0;
    font-style: italic;
  }

  /* Focus styles for accessibility */
  .primary-button:focus-visible,
  .secondary-button:focus-visible {
    outline: 2px solid rgb(34, 211, 238);
    outline-offset: 2px;
  }
</style>
