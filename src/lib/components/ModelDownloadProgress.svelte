<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { mustInvoke } from '$lib/ipc/invoke';
  import { listen } from '@tauri-apps/api/event';
  import ModalCloseButton from './ModalCloseButton.svelte';
  import HintTooltip from './settings/primitives/HintTooltip.svelte';
  import { modelStore } from '../stores/modelStore';
  import { get } from 'svelte/store';
  import {
    MODEL_REGISTRY,
    getAvailableModels,
    getModelById,
    formatSize,
    getSizeBadgeColor,
    ModelDialogState,
    type ModelDefinition,
    type ModelCategory,
    type CustomModelInfo,
  } from '../types/models';
  import { loadGpuContext, type GpuInfo, type ModelRecommendation } from '$lib/utils/GpuUtil';
  import { pageStore, showImportCustomModelModal } from '$lib/stores/pageStore';
  import { modelDownloadEvents } from '$lib/composables/useModelDownloadEvents';
  import { fade } from 'svelte/transition';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { cmdOpenDirectory, cmdGetModelsDir } from '$lib/ipc/commands';
  import { Icon } from '$lib/icons';
  import { useCopyFeedback } from '$lib/composables/useCopyFeedback';
  import {
    getCustomModelInfo,
    customModelChanged,
    importCustomModelFromUrl,
    getCustomModelDirPath,
  } from '$lib/services/customModelService';
  import { t } from '$lib/locales';
  import ModelCategoryTabs from './model-download/ModelCategoryTabs.svelte';
  import DownloadingState from './model-download/DownloadingState.svelte';
  import {
    isModelCompatible as _isModelCompatible,
    getGpuTooltip as _getGpuTooltip,
  } from '$lib/utils/modelCompatibility';

  type Props = {
    isDownloading?: boolean;
    /** Download progress 0-100 */
    progress?: number;
    downloadSpeed?: string;
    timeRemaining?: string;
    timeElapsed?: string;
    /** Error message if download failed */
    error?: string;
    /** Real-time status label */
    status?: string;
    onClose?: () => void;
    /**
     * Which model category this dialog operates on. Drives both the model list
     * filter and the active-model store. Default 'transcription' preserves
     * existing call-site behavior.
     */
    category?: ModelCategory;
    /**
     * Active-model store. Defaults to the transcription `modelStore`. Pass
     * `transformModelStore` for the transformation picker - both expose the
     * same shape (selectedModel, downloadedModels, selectModel, etc.).
     */
    activeStore?: typeof modelStore;
  };

  let {
    isDownloading = false,
    progress = 0,
    downloadSpeed = '',
    timeRemaining = '',
    timeElapsed = '',
    error = '',
    status = '',
    onClose = undefined,
    category = 'transcription',
    activeStore = modelStore,
  }: Props = $props();

  function closeModal() {
    if (onClose) {
      onClose();
    } else {
      isDownloading = false;
    }
  }

  let viewState: ModelDialogState = $state(ModelDialogState.Selection);
  // BYO custom-model downloads hand off to this SAME dialog (see
  // ImportCustomModelModal.svelte's downloadHfFile + beginCustomDownload) -
  // jump straight to the Downloading view instead of the registry Selection
  // list, which a custom download has nothing to do with.
  const isCustomDownload = modelDownloadEvents.isCustomDownload;
  const customDownloadInfo = modelDownloadEvents.customDownloadInfo;
  $effect(() => {
    if ($isCustomDownload) {
      viewState = ModelDialogState.Downloading;
      // checkProgressCompletion() (below) calls markModelDownloaded/selectModel
      // using tempSelectedModel once progress hits 100 - without this it would
      // still hold whatever registry model was last browsed, not 'custom'.
      tempSelectedModel = 'custom';
    }
  });
  // Bytes already on disk for a PAUSED BYO download (shown in the Ready-view
  // amber banner, same UX as the registry partial-download banner).
  let customPartialBytes = $state<number | null>(null);

  async function checkCustomPartial() {
    const dirName = get(customDownloadInfo)?.modelDirName;
    if (!dirName) {
      customPartialBytes = null;
      return;
    }
    try {
      customPartialBytes = await mustInvoke<number | null>('check_custom_model_partial_download', {
        modelDirName: dirName,
      });
    } catch (err) {
      console.error('Failed to check custom partial download:', err);
      customPartialBytes = null;
    }
  }

  async function deleteCustomPartial() {
    const dirName = get(customDownloadInfo)?.modelDirName;
    if (!dirName) return;
    try {
      await mustInvoke('delete_custom_model_partial_download', { modelDirName: dirName });
      customPartialBytes = null;
    } catch (err) {
      console.error('Failed to delete custom partial download:', err);
      error = `${err}`;
      errorType = 'delete';
      viewState = ModelDialogState.Error;
    }
  }

  /** Resume (or restart, if the partial was deleted) the paused BYO download
   * by re-invoking the same import call - `download_single_file` picks up
   * the existing `.partial` file automatically via HTTP Range. */
  async function resumeCustomDownload() {
    const info = get(customDownloadInfo);
    if (!info?.hfUrl || !info?.hfFilePath) return;
    error = '';
    try {
      await importCustomModelFromUrl(info.hfUrl, info.hfFilePath);
    } catch (err) {
      const msg = `${err}`;
      if (!msg.toLowerCase().includes('cancelled')) {
        error = msg;
        errorType = 'download';
        viewState = ModelDialogState.Error;
      }
    }
  }
  let tempSelectedModel = $state('distil-large-v3'); // Temporary selection for preview only
  let previousIsDownloading = false;
  let unlistenDownloadComplete: any = null;
  let unlistenDownloadError: any = null;
  let errorType: 'download' | 'delete' = $state('download'); // Track what operation failed
  let partialDownloadSize = $state<number | null>(null); // Size of partial download in bytes
  let gpuInfo: GpuInfo = $state({ name: null, vramGb: null, vramMb: null, displayString: null });
  let recommendation: ModelRecommendation | null = null;
  let ortAvailable: boolean = true; // Whether ONNX Runtime can load (false on macOS < 13.3)
  let hideIncompatible: boolean = $state(true); // Hide incompatible models by default
  let easternOnly: boolean = $state(false); // Filter to Eastern-language-capable models (off by default)

  // Local wrappers bind the pure utilities (in $lib/utils/modelCompatibility)
  // to the current component's reactive state, so the rest of the file (and
  // the child <ModelDownloadActions/> that takes them as props) doesn't have
  // to know about the dependency injection.
  const getGpuTooltip = (gpuRec: string) => _getGpuTooltip(gpuRec, category, t);
  const isModelCompatible = (model: ModelDefinition) =>
    _isModelCompatible(model, gpuInfo, ortAvailable);

  // Check for partial downloads when viewing model info
  async function checkPartialDownload(modelId: string) {
    try {
      const size = await mustInvoke<number | null>('check_partial_download', {
        modelType: modelId,
      });
      partialDownloadSize = size;
    } catch (err) {
      console.error('Failed to check partial download:', err);
      partialDownloadSize = null;
    }
  }

  // Delete partial download - stays on same screen, just clears partial indicator
  async function deletePartialDownload() {
    try {
      await mustInvoke('delete_partial_download', { modelType: tempSelectedModel });
      partialDownloadSize = null;
      // Stay on Ready screen - button will now show "Download" instead of "Resume"
    } catch (err) {
      console.error('Failed to delete partial:', err);
      error = `${err}`;
      errorType = 'delete';
      viewState = ModelDialogState.Error;
    }
  }

  // Use the active model store for state (transcription default, or transformation when passed)
  let selectedModel = $derived($activeStore.selectedModel);
  let downloadedModels = $derived($activeStore.downloadedModels);

  // Imported BYO model metadata - decorates the generic "Custom Model" row
  // with the real name and size. Refreshed on dialog open and whenever the
  // import modal closes (an import/replace/remove may have landed).
  let customInfo = $state<CustomModelInfo | null>(null);
  $effect(() => {
    $customModelChanged;
    if (isDownloading && !$showImportCustomModelModal) {
      void getCustomModelInfo().then(
        info => (customInfo = info),
        () => (customInfo = null)
      );
    }
  });

  // Get available models for the dialog's category (excluding coming soon, ORT-aware)
  let availableModels = $derived(
    getAvailableModels(ortAvailable, category).map(m => {
      if (m.id !== 'custom') return m;
      // Registry names are English-only literals; the custom row is the one
      // row whose copy must be localized here (imported or not).
      return customInfo
        ? {
            ...m,
            name: customInfo.name,
            sizeBytes: customInfo.sizeBytes,
            description: t('models.customImportedDesc'),
            gpuRecommendation: t('models.customBadge'),
          }
        : { ...m, name: t('customModel.title'), description: t('models.customDesc') };
    })
  );

  // Filtered models based on compatibility and optional Eastern language filter
  let filteredModels = $derived.by(() => {
    let models = hideIncompatible
      ? availableModels.filter(m => isModelCompatible(m))
      : availableModels;
    if (easternOnly && category === 'transcription') {
      models = models.filter(m => m.supportsEasternLanguages === true);
    }
    return models;
  });

  // Check if a model is the GPU-recommended one
  function isRecommendedForGpu(modelId: string): boolean {
    return recommendation?.model_id === modelId;
  }

  // Load downloaded status on mount
  onMount(async () => {
    try {
      // Initialize temp selection to match current selectedModel
      tempSelectedModel = selectedModel;

      // Load GPU context (info + recommendation + ORT status) - single source of truth
      const gpuContext = await loadGpuContext();
      gpuInfo = gpuContext.gpuInfo;
      recommendation = gpuContext.recommendation;
      ortAvailable = gpuContext.ortAvailable;

      // Listen for model download complete event
      unlistenDownloadComplete = await listen('model-download-complete', async (event: any) => {
        const downloadedModelId = event.payload;

        // Only react to completions for *this* dialog's category - the other
        // dialog instance (transcription vs transformation) is also listening.
        const completedDef = getModelById(downloadedModelId);
        if (completedDef && completedDef.category !== category) return;

        // Update the active store with the newly downloaded model
        await activeStore.selectModel(downloadedModelId);
        activeStore.markModelDownloaded(downloadedModelId);

        // Keep temp in sync
        tempSelectedModel = downloadedModelId;
      });

      // Listen for model download errors
      unlistenDownloadError = await listen('model-download-error', async (event: any) => {
        // Set error state and show error view
        error = event.payload || t('modelDownload.downloadFailed');
        errorType = 'download';
        viewState = ModelDialogState.Error;

        // Reset progress state
        progress = 0;
        downloadSpeed = '';
        timeRemaining = '';
        timeElapsed = '';
        status = '';
      });
    } catch (err) {
      console.error('Failed to initialize model dialog:', err);
    }
  });

  // Cleanup listeners on destroy
  onDestroy(() => {
    if (unlistenDownloadComplete) {
      unlistenDownloadComplete();
    }
    if (unlistenDownloadError) {
      unlistenDownloadError();
    }
    if (completionTimeout) {
      clearTimeout(completionTimeout);
    }
    sourceCopyFeedback.cleanup();
  });

  // Handle model selection (temporary preview only)
  function selectModel(modelId: string) {
    const model = getModelById(modelId);
    if (!model || model.isComingSoon) return;

    // BYO custom model: never downloaded via this dialog, and never shown in
    // the generic Info view. The Import Custom Model modal is the single
    // management surface (import + Use/Replace/Remove).
    if (modelId === 'custom') {
      // Seamless handoff: the import modal mounts at full opacity (fadeMs 0)
      // in the same update flush that unmounts this dialog - one repaint,
      // identical backdrops, so nothing behind flashes or pulses. returnTo
      // reopens this picker when that modal closes (ESC/X = back one layer).
      pageStore.openImportCustomModelModal({
        returnTo: () => modelDownloadEvents.openModal(category),
      });
      closeModal();
      return;
    }

    tempSelectedModel = modelId;
    partialDownloadSize = null; // Reset partial status

    if (downloadedModels[modelId]) {
      // Model already downloaded, show info screen
      viewState = ModelDialogState.Info;
    } else {
      // Model not downloaded, check for partial and show ready to download
      checkPartialDownload(modelId);
      viewState = ModelDialogState.Ready;
    }
  }

  // Start download
  async function startDownload() {
    try {
      // Set downloading state BEFORE invoking download
      viewState = ModelDialogState.Downloading;
      progress = 0;
      error = '';
      status = partialDownloadSize ? t('modelDownload.resuming') : t('modelDownload.starting'); // Set initial status immediately
      downloadSpeed = '';
      timeRemaining = '';
      timeElapsed = '';

      // Start download - this will trigger progress events
      await mustInvoke('download_whisper_model', { modelType: tempSelectedModel });
    } catch (err: any) {
      console.error('Download error:', err);
      if (!err.toString().includes('cancelled')) {
        error = err.toString().replace('Error: ', '');
        viewState = ModelDialogState.Error;
      }
    }
  }

  // Use selected model (confirm selection)
  async function useModel() {
    try {
      // Update the model selection via the active store
      await activeStore.selectModel(tempSelectedModel);

      // Close dialog
      closeModal();
    } catch (err) {
      console.error('Failed to update model settings:', err);
    }
  }

  // Delete model
  function deleteModel() {
    viewState = ModelDialogState.ConfirmDelete;
  }

  /**
   * Pick a fallback model when the user deletes the currently-active one.
   * Transform always falls back to the bundled tiny (always present);
   * voice picks any other downloaded model, preferring the recommended
   * one. Returns null if no fallback exists (caller leaves selection
   * as-is so the bottom-bar FTUE indicator shows "Select AI Model").
   */
  function pickFallbackAfterDelete(deletedId: string): string | null {
    // Both categories follow the same DRY rule now: pick any other
    // downloaded model in the same category; null if nothing remains
    // (UI surfaces the "Setup" / "Select AI Model" indicator).
    const stillDownloaded = Object.entries(downloadedModels)
      .filter(([id, exists]) => exists && id !== deletedId)
      .map(([id]) => id);
    if (stillDownloaded.length === 0) return null;
    // Prefer the recommended model if it's still on disk.
    const rec = MODEL_REGISTRY.find(m => m.category === category && m.isRecommended);
    if (rec && stillDownloaded.includes(rec.id)) return rec.id;
    return stillDownloaded[0];
  }

  // Confirm deletion
  async function confirmDelete() {
    const wasActive = selectedModel === tempSelectedModel;
    const deletedId = tempSelectedModel;
    try {
      await mustInvoke('delete_model', { modelType: tempSelectedModel });

      // Refresh canonical state from disk so the picker doesn't keep
      // showing a stale "Downloaded" badge after reopening the dialog.
      await activeStore.refreshDownloadedModels();

      // If we deleted the currently-active model, auto-switch so
      // settings + bottom-bar indicators reflect reality immediately.
      if (wasActive) {
        const fallback = pickFallbackAfterDelete(deletedId);
        if (fallback) {
          await activeStore.selectModel(fallback);
          tempSelectedModel = fallback;
        }
        // No fallback: leave selectedModel as-is. UI surfaces the
        // broken state via the existing FTUE indicator.
      }

      // Go back to selection
      viewState = ModelDialogState.Selection;
    } catch (err) {
      console.error('Failed to delete model:', err);
      error = `${err}`;
      errorType = 'delete';
      viewState = ModelDialogState.Error;
    }
  }

  // Cancel deletion
  function cancelDelete() {
    viewState = ModelDialogState.Info;
  }

  // Cancel download and return to appropriate state
  async function cancelDownload() {
    try {
      await mustInvoke('cancel_model_download');
    } catch (err) {
      console.error('Failed to cancel download:', err);
    }

    // Reset download state
    progress = 0;
    downloadSpeed = '';
    timeRemaining = '';
    timeElapsed = '';
    status = '';
    isCompleting = false;
    isFadingOut = false;
    if (completionTimeout) {
      clearTimeout(completionTimeout);
      completionTimeout = null;
    }

    // BYO custom downloads have no registry entry - never route to the
    // generic Info/Ready views (they'd render MODEL_REGISTRY's 'custom'
    // placeholder: 0 bytes, "varies" everywhere). The Ready template
    // branches on tempSelectedModel === 'custom' to show a minimal,
    // real-data-only paused card instead.
    if (tempSelectedModel === 'custom') {
      await checkCustomPartial();
      viewState = ModelDialogState.Ready;
      return;
    }

    // Return to appropriate view based on whether model exists
    if (downloadedModels[tempSelectedModel]) {
      viewState = ModelDialogState.Info;
    } else {
      // Re-check disk so the Ready screen shows "Resume Download" / partial-size
      // banner instead of stale "Download" from before the pause.
      await checkPartialDownload(tempSelectedModel);
      viewState = ModelDialogState.Ready;
    }
  }

  // Try again after error
  async function tryAgain() {
    error = '';
    progress = 0;
    if (tempSelectedModel === 'custom') {
      await checkCustomPartial();
      viewState = ModelDialogState.Ready;
      return;
    }
    // Same reason as cancelDownload: a partial may exist now after the failed run.
    await checkPartialDownload(tempSelectedModel);
    viewState = ModelDialogState.Ready;
  }

  // Handle progress completion with animated sequence
  let completionTimeout: ReturnType<typeof setTimeout> | null = null;
  let isCompleting = $state(false);
  let isFadingOut = $state(false);

  function checkProgressCompletion(currentProgress: number) {
    if (
      currentProgress >= 100 &&
      viewState === ModelDialogState.Downloading &&
      !completionTimeout
    ) {
      // Mark as downloaded immediately
      activeStore.markModelDownloaded(tempSelectedModel);
      activeStore.selectModel(tempSelectedModel);

      // Phase 1: Wait for clip-path to reach 100%, then turn green + pulse
      completionTimeout = setTimeout(() => {
        isCompleting = true;

        // Phase 2: After pulse completes, fade out
        setTimeout(() => {
          isFadingOut = true;

          // Phase 3: Switch to success view
          setTimeout(() => {
            viewState = ModelDialogState.Success;
            completionTimeout = null;
            isCompleting = false;
            isFadingOut = false;
          }, 400);
        }, 1000);
      }, 700);
    }
  }

  // Watch progress changes
  $effect(() => {
    progress;
    checkProgressCompletion(progress);
  });

  // Track dialog open/close and reset state appropriately
  $effect(() => {
    if (isDownloading && !previousIsDownloading) {
      // A BYO custom download owns viewState via the isCustomDownload effect
      // above (forces Downloading) - this effect's own Selection/Downloading
      // decision (based on registry progress/status) must not run at all here,
      // or it races that effect and can clobber Downloading back to Selection
      // on the very first tick, before progress/status have any value yet.
      if (!$isCustomDownload) {
        // Dialog just opened - check if there's an active download in progress
        // Active = progress between 1-99, OR a non-complete status message
        const isActiveStatus = status && status.length > 0 && status !== 'Complete';
        const hasActiveDownload = (progress >= 1 && progress < 100) || isActiveStatus;

        if (hasActiveDownload) {
          // Active download in progress - show the downloading view
          viewState = ModelDialogState.Downloading;
        } else {
          // No active download - reset to selection view
          viewState = ModelDialogState.Selection;
          tempSelectedModel = selectedModel;
          error = '';
        }
      }
      // Always re-check disk on open so stale state from another session
      // (eg user manually deleted files, or settings drifted) can't show
      // a model as "Downloaded" when it isn't.
      activeStore.refreshDownloadedModels();
      previousIsDownloading = true;
    } else if (!isDownloading && previousIsDownloading) {
      previousIsDownloading = false;
    }
  });

  // Get the currently selected temp model definition
  let tempModel = $derived(getModelById(tempSelectedModel));

  // Source URL for the "verify where the bytes come from" link next to a
  // model's name - null when there's nothing meaningful to link to (local-file
  // BYO imports have no repo to point at).
  const registrySourceUrl = $derived(
    tempSelectedModel !== 'custom' && tempModel && tempModel.repoId
      ? `https://huggingface.co/${tempModel.repoId}`
      : null
  );

  /** Normalize a pasted repo/URL input + resolved file path into a browsable
   * HF blob link, matching how the backend resolves the same input. */
  function hfBlobUrl(input: string, filePath: string): string {
    const repo = input
      .replace(/^https?:\/\/huggingface\.co\//i, '')
      .replace(/\/(blob|resolve)\/.*$/i, '')
      .trim();
    return `https://huggingface.co/${repo}/blob/main/${filePath}`;
  }

  const customSourceUrl = $derived(
    $customDownloadInfo?.hfUrl && $customDownloadInfo?.hfFilePath
      ? hfBlobUrl($customDownloadInfo.hfUrl, $customDownloadInfo.hfFilePath)
      : null
  );

  // Name + source link shown in the Downloading header - same clickable
  // "verify where the bytes come from" pattern as the Ready/paused cards.
  const downloadName = $derived(
    tempSelectedModel === 'custom'
      ? ($customDownloadInfo?.name ?? t('customModel.title'))
      : (tempModel?.name ?? '')
  );
  const downloadSourceUrl = $derived(
    tempSelectedModel === 'custom' ? customSourceUrl : registrySourceUrl
  );

  // Clicking the "Downloading {name}" header link copies the source URL
  // instead of opening it - a quick way to verify/paste it elsewhere while
  // the download is still running.
  const sourceCopyFeedback = useCopyFeedback();
  const { isCopied: isSourceUrlCopied } = sourceCopyFeedback;

  /** "Open download folder" - custom BYO downloads get their own dedicated
   * directory (resolved via the backend); registry downloads share one HF
   * cache root, same as the "open models folder" button in Settings. */
  async function openDownloadDir() {
    try {
      const dir =
        tempSelectedModel === 'custom' && $customDownloadInfo?.modelDirName
          ? await getCustomModelDirPath($customDownloadInfo.modelDirName)
          : await cmdGetModelsDir();
      await cmdOpenDirectory(dir);
    } catch (err) {
      console.error('Failed to open download directory:', err);
    }
  }
</script>

{#if isDownloading}
  <div class="modal-backdrop">
    <div class="modal-container">
      <!-- Close/Minimize Button (top-right). Downloading state renders its
           own minimize button (DownloadingState.svelte, onMinimize prop below)
           to stay 1:1 with the BYO modal's shared component - nothing to
           render here for that state. -->
      {#if viewState === ModelDialogState.ConfirmDelete}
        <!-- X in delete confirmation goes back to Info, not close all -->
        <ModalCloseButton onClose={cancelDelete} />
      {:else if viewState !== ModelDialogState.Downloading}
        <ModalCloseButton onClose={closeModal} />
      {/if}

      <!-- Header -->
      <header class="modal-header">
        <div
          class="header-icon-container {viewState === ModelDialogState.Error
            ? 'error'
            : viewState === ModelDialogState.Success
              ? 'success'
              : ''}"
        >
          {#if viewState === ModelDialogState.Error}
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <path
                fill-rule="evenodd"
                d="M5.47 5.47a.75.75 0 011.06 0L12 10.94l5.47-5.47a.75.75 0 111.06 1.06L13.06 12l5.47 5.47a.75.75 0 11-1.06 1.06L12 13.06l-5.47 5.47a.75.75 0 01-1.06-1.06L10.94 12 5.47 6.53a.75.75 0 010-1.06z"
                clip-rule="evenodd"
              />
            </svg>
          {:else if viewState === ModelDialogState.Success}
            <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24">
              <path
                fill-rule="evenodd"
                d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.739a.75.75 0 011.04-.208z"
                clip-rule="evenodd"
              />
            </svg>
          {:else}
            <svg
              class="w-4 h-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              stroke-width="1.5"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"
              />
            </svg>
          {/if}
        </div>

        <h3 class="header-title">
          {#if viewState === ModelDialogState.Error}
            {errorType === 'delete'
              ? t('modelDownload.deleteFailed')
              : t('modelDownload.downloadFailed')}
          {:else if viewState === ModelDialogState.Success}
            {t('modelDownload.downloadComplete')}
          {:else if viewState === ModelDialogState.Downloading}
            {t('modelDownload.downloadingLabel')}
            {#if downloadSourceUrl}
              <button
                type="button"
                class="model-name-link header-name-link"
                title={`${downloadSourceUrl}\n${t('common.clickToCopy')}`}
                onclick={() => sourceCopyFeedback.copy(downloadSourceUrl)}
              >
                {downloadName}
              </button>
              {#if $isSourceUrlCopied}
                <span class="copied-badge" transition:fade={{ duration: 150 }}>
                  {t('common.copied')}
                </span>
              {/if}
            {:else}
              {downloadName}
            {/if}
          {:else if viewState === ModelDialogState.Ready && tempSelectedModel === 'custom'}
            {t('customModel.pausedTitle')}
          {:else if viewState === ModelDialogState.Ready}
            {t('modelDownload.readyToDownload')}
          {:else if viewState === ModelDialogState.Info && tempSelectedModel !== 'custom'}
            {t('modelDownload.modelInfo')}
          {:else if viewState === ModelDialogState.ConfirmDelete}
            {t('modelDownload.deleteModel')}
          {:else}
            {t('modelDownload.selectModel')}
            <span class="header-tagline">{t('modelDownload.selectModelTagline')}</span>
          {/if}
        </h3>
      </header>

      {#if viewState === ModelDialogState.Selection}
        <ModelCategoryTabs
          {category}
          {filteredModels}
          {downloadedModels}
          {selectedModel}
          {hideIncompatible}
          {easternOnly}
          hasVram={!!gpuInfo.vramMb}
          gpuDisplayString={gpuInfo.displayString}
          onHideIncompatibleChange={v => {
            hideIncompatible = v;
          }}
          onEasternOnlyChange={v => {
            easternOnly = v;
          }}
          onSelectModel={selectModel}
          {isModelCompatible}
          {isRecommendedForGpu}
          {getGpuTooltip}
        />
      {:else if viewState === ModelDialogState.Ready && tempSelectedModel === 'custom'}
        <!-- BYO paused download: only real, known data - no fabricated
             registry fields (speed/vram/gpu/languages don't exist for a
             custom import), no fake "Downloaded"/"Active". -->
        <div class="space-y-4">
          <div class="info-card">
            <div class="flex items-center gap-2 flex-wrap mb-2">
              {#if customSourceUrl}
                <button
                  type="button"
                  class="model-name-link"
                  title={customSourceUrl}
                  onclick={() => launchUrl(customSourceUrl)}
                >
                  {$customDownloadInfo?.name ?? t('customModel.title')}
                </button>
              {:else}
                <span class="text-base font-medium text-white">
                  {$customDownloadInfo?.name ?? t('customModel.title')}
                </span>
              {/if}
              {#if $customDownloadInfo?.totalBytes}
                <span
                  class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs bg-zinc-700/60 text-zinc-300"
                >
                  {formatSize($customDownloadInfo.totalBytes)}
                </span>
              {/if}
            </div>
            <p class="text-sm text-zinc-400 mb-0">{t('customModel.pausedHint')}</p>
          </div>

          {#if customPartialBytes}
            <div
              class="flex items-center justify-between p-2 bg-amber-900/20 border border-amber-600/30 rounded-lg text-xs"
            >
              <span class="text-amber-400">
                {t('modelDownload.partialDownload', { size: formatSize(customPartialBytes) })}
              </span>
              <button
                onclick={deleteCustomPartial}
                class="text-amber-400 hover:text-white underline cursor-pointer transition-colors"
              >
                {t('modelDownload.deleteButton')}
              </button>
            </div>
          {/if}

          <div class="flex gap-2">
            <button onclick={closeModal} class="btn-secondary" style="flex: 1">
              {t('common.close')}
            </button>
            <button onclick={resumeCustomDownload} class="btn-primary" style="flex: 2">
              {customPartialBytes
                ? `${t('modelDownload.resume')} (${(customPartialBytes / 1e9).toFixed(1)} GB)`
                : t('modelDownload.download')}
            </button>
          </div>
        </div>
      {:else if viewState === ModelDialogState.Ready && tempModel}
        <!-- Ready to Download -->
        <div class="space-y-4">
          <div class="info-card">
            <!-- Name + badges + external link -->
            <div class="flex items-center gap-2 flex-wrap mb-2">
              {#if registrySourceUrl}
                <button
                  type="button"
                  class="model-name-link"
                  title={registrySourceUrl}
                  onclick={() => launchUrl(registrySourceUrl)}
                >
                  {tempModel.name}
                </button>
              {:else}
                <span class="text-base font-medium text-white">{tempModel.name}</span>
              {/if}

              <span
                class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs {getSizeBadgeColor(
                  tempModel.sizeCategory
                )}"
              >
                {formatSize(tempModel.sizeBytes)}
              </span>

              {#if tempModel.isNew}
                <span class="badge-new">{t('modelDownload.new')}</span>
              {/if}
            </div>

            <!-- Description + hint -->
            <p class="text-sm text-zinc-400 mb-3">
              {tempModel.description}
              {#if tempModel.hint}
                <HintTooltip hint={tempModel.hint} />
              {/if}
            </p>

            <div class="grid grid-cols-2 gap-2 text-xs">
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.speed')}</span>
                <span class="text-zinc-200 font-medium"
                  >{tempModel.realtimeSpeed} {t('modelDownload.realtime')}</span
                >
              </div>
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.minVram')}</span>
                <span class="text-zinc-200 font-medium">{tempModel.minVram}</span>
              </div>
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.gpu')}</span>
                {#if getGpuTooltip(tempModel.gpuRecommendation)}
                  <span
                    class="text-zinc-200 font-medium gpu-hint"
                    title={getGpuTooltip(tempModel.gpuRecommendation)}
                    >{tempModel.gpuRecommendation}</span
                  >
                {:else}
                  <span class="text-zinc-200 font-medium">{tempModel.gpuRecommendation}</span>
                {/if}
              </div>
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.languages')}</span>
                <span
                  class="text-zinc-200 font-medium"
                  class:cursor-help={!!tempModel.languagesFull}
                  title={tempModel.languagesFull}>{tempModel.languages || 'Multi'}</span
                >
              </div>
            </div>
          </div>

          {#if partialDownloadSize}
            <div
              class="flex items-center justify-between p-2 bg-amber-900/20 border border-amber-600/30 rounded-lg text-xs"
            >
              <span class="text-amber-400">
                {t('modelDownload.partialDownload', { size: formatSize(partialDownloadSize) })}
              </span>
              <button
                onclick={deletePartialDownload}
                class="text-amber-400 hover:text-white underline cursor-pointer transition-colors"
              >
                {t('modelDownload.deleteButton')}
              </button>
            </div>
          {/if}

          <div class="flex gap-2">
            <button
              onclick={() => (viewState = ModelDialogState.Selection)}
              class="btn-secondary"
              style="flex: 1"
            >
              {t('common.back')}
            </button>
            <button onclick={startDownload} class="btn-primary" style="flex: 2">
              {partialDownloadSize
                ? `${t('modelDownload.resume')} (${(partialDownloadSize / 1e9).toFixed(1)} GB)`
                : t('modelDownload.download')}
            </button>
          </div>
        </div>
      {:else if viewState === ModelDialogState.Downloading}
        <!-- Downloading Progress: shared component (DownloadingState.svelte),
             also used by ImportCustomModelModal.svelte's BYO download flow -
             one component so the two flows can't silently drift apart again. -->
        <DownloadingState
          {progress}
          speed={downloadSpeed}
          {timeRemaining}
          {status}
          totalBytes={tempSelectedModel === 'custom'
            ? ($customDownloadInfo?.totalBytes ?? 0)
            : (tempModel?.sizeBytes ?? 0)}
          {isCompleting}
          {isFadingOut}
          onCancel={cancelDownload}
          onMinimize={closeModal}
          onOpenDir={openDownloadDir}
          openDirLabel={t('modelDownload.openDownloadDir')}
        />
      {:else if viewState === ModelDialogState.Success}
        <!-- Success State (bar already faded out from completion animation) -->
        <div class="space-y-4">
          <div class="text-center py-4">
            <p class="text-green-400 text-sm font-medium">
              {t('model.modelReady')}
            </p>
          </div>
          <button onclick={closeModal} class="btn-success-done w-full">
            {t('settingsFooter.done')}
          </button>
        </div>
      {:else if viewState === ModelDialogState.Error}
        <!-- Error State -->
        <div class="space-y-3 py-2">
          {#if error}
            <p class="text-zinc-400 text-sm text-center">{error}</p>
          {/if}
          <div class="flex justify-center">
            <button
              onclick={errorType === 'delete'
                ? () => (viewState = ModelDialogState.Info)
                : tryAgain}
              class={errorType === 'delete' ? 'btn-secondary' : 'btn-danger'}
            >
              {errorType === 'delete' ? t('common.back') : t('modelDownload.tryAgain')}
            </button>
          </div>
        </div>
      {:else if viewState === ModelDialogState.Info && tempModel && tempSelectedModel !== 'custom'}
        <!-- Model Info (Already Downloaded). Never reached for
             'custom' - cancelDownload/tryAgain route it to the BYO Ready
             branch above instead, since this view renders MODEL_REGISTRY's
             placeholder fields (0 bytes, "varies") for that id. -->
        <div class="space-y-4">
          <div class="info-card">
            <div class="flex items-center gap-2 flex-wrap mb-2">
              {#if registrySourceUrl}
                <button
                  type="button"
                  class="model-name-link"
                  title={registrySourceUrl}
                  onclick={() => launchUrl(registrySourceUrl)}
                >
                  {tempModel.name}
                </button>
              {:else}
                <span class="text-base font-medium text-white">{tempModel.name}</span>
              {/if}

              <span
                class="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-xs {getSizeBadgeColor(
                  tempModel.sizeCategory
                )}"
              >
                {formatSize(tempModel.sizeBytes)}
              </span>

              {#if tempModel.isNew}
                <span class="badge-new">{t('modelDownload.new')}</span>
              {/if}
            </div>

            <div class="flex items-center gap-4 mb-3">
              {#if tempModel?.bundled}
                <!-- Bundled = ships with the app, never user-downloaded.
                     Distinct from "Downloaded" so the absence of the
                     Delete button below is self-evident, not mysterious. -->
                <div class="flex items-center gap-1.5">
                  <svg
                    class="w-4 h-4 text-zinc-400"
                    fill="none"
                    viewBox="0 0 24 24"
                    stroke="currentColor"
                    stroke-width="2"
                  >
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"
                    />
                  </svg>
                  <span class="text-zinc-300 text-sm">{t('modelDownload.bundled')}</span>
                </div>
              {:else}
                <div class="flex items-center gap-1.5">
                  <svg class="w-4 h-4 text-green-400" fill="currentColor" viewBox="0 0 24 24">
                    <path
                      fill-rule="evenodd"
                      d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.739a.75.75 0 011.04-.208z"
                      clip-rule="evenodd"
                    />
                  </svg>
                  <span class="text-green-400 text-sm">{t('modelDownload.downloaded')}</span>
                </div>
              {/if}
              {#if selectedModel === tempSelectedModel}
                <div class="flex items-center gap-1.5">
                  <svg class="w-4 h-4 text-lime-400" fill="currentColor" viewBox="0 0 24 24">
                    <path
                      fill-rule="evenodd"
                      d="M19.916 4.626a.75.75 0 01.208 1.04l-9 13.5a.75.75 0 01-1.154.114l-6-6a.75.75 0 011.06-1.06l5.353 5.353 8.493-12.739a.75.75 0 011.04-.208z"
                      clip-rule="evenodd"
                    />
                  </svg>
                  <span class="text-lime-400 text-sm">{t('modelDownload.active')}</span>
                </div>
              {/if}
            </div>

            <!-- Description + hint -->
            <p class="text-sm text-zinc-400 mb-3">
              {tempModel.description}
              {#if tempModel.hint}
                <HintTooltip hint={tempModel.hint} />
              {/if}
              {#if tempModel?.bundled}
                <span class="block text-xs text-zinc-500 italic mt-1"
                  >{t('modelDownload.bundledNote')}</span
                >
              {/if}
            </p>

            <div class="grid grid-cols-2 gap-2 text-xs">
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.speed')}</span>
                <span class="text-zinc-200 font-medium"
                  >{tempModel.realtimeSpeed} {t('modelDownload.realtime')}</span
                >
              </div>
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.minVram')}</span>
                {#if getGpuTooltip(tempModel.minVram)}
                  <span
                    class="text-zinc-200 font-medium gpu-hint"
                    title={getGpuTooltip(tempModel.minVram)}>{tempModel.minVram}</span
                  >
                {:else}
                  <span class="text-zinc-200 font-medium">{tempModel.minVram}</span>
                {/if}
              </div>
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.gpu')}</span>
                {#if getGpuTooltip(tempModel.gpuRecommendation)}
                  <span
                    class="text-zinc-200 font-medium gpu-hint"
                    title={getGpuTooltip(tempModel.gpuRecommendation)}
                    >{tempModel.gpuRecommendation}</span
                  >
                {:else}
                  <span class="text-zinc-200 font-medium">{tempModel.gpuRecommendation}</span>
                {/if}
              </div>
              <div class="stat-box">
                <span class="text-zinc-500 block">{t('modelDownload.languages')}</span>
                <span
                  class="text-zinc-200 font-medium"
                  class:cursor-help={!!tempModel.languagesFull}
                  title={tempModel.languagesFull}>{tempModel.languages || 'Multi'}</span
                >
              </div>
            </div>
          </div>

          <div class="flex gap-2">
            <button
              onclick={() => (viewState = ModelDialogState.Selection)}
              class="btn-secondary"
              style="flex: 1"
            >
              {t('common.back')}
            </button>
            <button onclick={useModel} class="btn-primary" style="flex: 2">
              {selectedModel === tempSelectedModel
                ? t('common.close')
                : t('modelDownload.useModel')}
            </button>
          </div>

          {#if !tempModel?.bundled}
            <button onclick={deleteModel} class="btn-ghost-danger w-full">
              {t('modelDownload.deleteModel')}
            </button>
          {/if}
        </div>
      {:else if viewState === ModelDialogState.ConfirmDelete && tempModel}
        <!-- Delete Confirmation - minimal -->
        <div class="space-y-4">
          <p class="text-zinc-300 text-sm">
            <strong class="text-white">{tempModel.name}</strong>
            <span class="text-zinc-500 ml-1">({formatSize(tempModel.sizeBytes)})</span>
          </p>

          <div class="flex gap-2">
            <button onclick={cancelDelete} class="btn-secondary">
              {t('common.cancel')}
            </button>
            <button onclick={confirmDelete} class="btn-danger">
              {t('common.delete')}
            </button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  /* Modal Container */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    /* Solid dark background - no expensive backdrop-filter */
    background: rgba(0, 0, 0, 0.85);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    cursor: default;
    /* Isolate to prevent repaints affecting main UI */
    contain: layout style paint;
    isolation: isolate;
  }

  .modal-container {
    position: relative;
    /* Solid background - no backdrop-filter needed */
    background: rgba(24, 24, 27, 0.98);
    border-radius: 1rem;
    border: 1px solid rgba(255, 255, 255, 0.06);
    padding: 1.5rem;
    max-width: 28rem;
    width: 100%;
    margin: 1rem;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
    cursor: default;
  }

  /* Header - matches SettingsHeader style */
  .modal-header {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding-bottom: 1rem;
    margin-bottom: 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .header-title {
    flex: 1;
    font-size: 0.9375rem;
    font-weight: 500;
    color: #e4e4e7;
    letter-spacing: -0.01em;
    margin: 0;
  }

  .header-tagline {
    margin-left: 0.625rem;
    font-size: 0.6875rem;
    font-weight: 400;
    color: #52525b;
    letter-spacing: 0;
  }

  /* Header Icon */
  .header-icon-container {
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    color: #52525b;
    transition: color 50ms ease;
  }

  .modal-header:hover .header-icon-container {
    color: #71717a;
  }

  .header-icon-container.error {
    color: #ef4444;
  }

  .header-icon-container.success {
    color: #84cc16;
  }

  /* Buttons - snappy, no transforms */
  .btn-primary {
    flex: 1;
    padding: 0.625rem 1rem;
    background: #84cc16;
    color: #18181b;
    border-radius: 0.5rem;
    font-weight: 500;
    font-size: 0.875rem;
    cursor: pointer;
    border: none;
    transition: background-color 50ms ease;
  }

  .btn-primary:hover {
    background: #a3e635;
  }

  .btn-primary:active {
    background: #65a30d;
  }

  /* Done button on success screen - subtle green to complement checkmark without clashing */
  .btn-success-done {
    padding: 0.625rem 1rem;
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
    border: 1px solid rgba(34, 197, 94, 0.3);
    border-radius: 0.5rem;
    font-weight: 500;
    font-size: 0.875rem;
    cursor: pointer;
    transition:
      background-color 100ms ease,
      border-color 100ms ease;
  }

  .btn-success-done:hover {
    background: rgba(34, 197, 94, 0.25);
    border-color: rgba(34, 197, 94, 0.5);
  }

  .btn-success-done:active {
    background: rgba(34, 197, 94, 0.35);
  }

  .btn-secondary {
    flex: 1;
    padding: 0.625rem 1rem;
    background: #27272a;
    color: #d4d4d8;
    border-radius: 0.5rem;
    font-weight: 500;
    font-size: 0.875rem;
    cursor: pointer;
    border: none;
    transition: background-color 50ms ease;
  }

  .btn-secondary:hover {
    background: #3f3f46;
  }

  .btn-secondary:active {
    background: #18181b;
  }

  .btn-danger {
    padding: 0.625rem 1.5rem;
    background: #dc2626;
    color: white;
    border-radius: 0.5rem;
    font-weight: 500;
    font-size: 0.875rem;
    cursor: pointer;
    border: none;
    transition: background-color 50ms ease;
  }

  .btn-danger:hover {
    background: #ef4444;
  }

  .btn-danger:active {
    background: #b91c1c;
  }

  .btn-ghost-danger {
    padding: 0.5rem 1rem;
    background: transparent;
    color: #f87171;
    border-radius: 0.5rem;
    font-size: 0.875rem;
    cursor: pointer;
    border: none;
    transition:
      background-color 50ms ease,
      color 50ms ease;
  }

  .btn-ghost-danger:hover {
    background: rgba(239, 68, 68, 0.1);
    color: #fca5a5;
  }

  /* Info Card - Flat, minimal */
  .info-card {
    background: rgba(255, 255, 255, 0.02);
    border-radius: 0.75rem;
    padding: 1rem;
    border: 1px solid rgba(255, 255, 255, 0.04);
  }

  /* "Verify where the bytes come from" - underlined + clickable model name,
     opens the HF source page. Bare text otherwise (no known source URL). */
  .model-name-link {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: 1rem;
    font-weight: 500;
    color: #ffffff;
    text-decoration: underline;
    text-decoration-color: rgba(255, 255, 255, 0.35);
    cursor: pointer;
    transition: text-decoration-color 100ms ease;
  }

  /* In the header, inherit the h3's own size/weight instead of the info-card size. */
  .header-name-link {
    font-size: inherit;
    font-weight: inherit;
  }

  .copied-badge {
    margin-left: 0.5rem;
    font-size: 0.6875rem;
    font-weight: 500;
    color: #4ade80;
    vertical-align: middle;
  }

  .model-name-link:hover {
    text-decoration-color: rgba(255, 255, 255, 0.9);
  }

  /* Stat boxes in info cards */
  .stat-box {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 0.5rem;
    padding: 0.5rem 0.75rem;
  }

  .badge-new {
    font-size: 0.5625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    background: rgba(59, 130, 246, 0.1);
    color: #60a5fa;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .badge-coming-soon {
    font-size: 0.5625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    background: rgba(113, 113, 122, 0.1);
    color: #71717a;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* Hint tooltip (same as SettingRow) */
  .hint-trigger {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: help;
    margin-left: 0.25rem;
    vertical-align: middle;
  }

  /* GPU hint - dashed underline with native tooltip */
  .gpu-hint {
    text-decoration: underline;
    text-decoration-style: dashed;
    text-underline-offset: 2px;
    cursor: help;
    color: #a1a1aa;
  }

  .gpu-hint:hover {
    color: #d4d4d8;
  }
</style>
