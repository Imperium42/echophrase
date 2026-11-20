// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Model download events composable (singleton)
 *
 * Holds page-level state for the *single* download UI shared across both
 * transcription (voice) and transformation (grammar) categories. The same
 * Tauri events (`model-download-*`) are used regardless of category - we
 * route store updates by looking up the model's `category` in the registry.
 *
 * Why a singleton: the dialog must persist when modals close (eg user
 * minimises the picker, closes Settings). Mounting it at the page level
 * with module-scope state makes that automatic.
 */

import { writable, type Readable, type Writable, get } from 'svelte/store';
import { modelStore } from '$lib/stores/modelStore';
import { transformModelStore } from '$lib/stores/transformModelStore';
import { getModelById, type ModelCategory } from '$lib/types/models';
import { translate } from '$lib/locales';
import type { listen as tauriListen } from '@tauri-apps/api/event';

type TauriListen = typeof tauriListen;

/**
 * Everything the shared dialog needs to render a BYO download without
 * fabricating registry fields (size/speed/vram/etc) and to Resume a paused
 * one. Set synchronously by `beginCustomDownload`; `modelDirName` is filled
 * in slightly later once the backend's `model-download-start` event (which
 * knows the actual on-disk directory name) arrives.
 */
export interface CustomDownloadInfo {
  name: string;
  totalBytes: number;
  /** HF repo input + resolved file path, for Resume. Undefined for local-file imports (nothing to re-fetch). */
  hfUrl?: string;
  hfFilePath?: string;
  /** On-disk directory basename, for check/delete-partial-download commands. */
  modelDirName?: string;
}

export interface ModelDownloadState {
  /** Whether the download modal is visible (writable for bind:) */
  isModalOpen: Writable<boolean>;
  /** Whether a download is actively in progress */
  isDownloadActive: Readable<boolean>;
  /** Whether the active download is a BYO custom-model import */
  isCustomDownload: Readable<boolean>;
  /** Metadata for the active/paused BYO download, or null when none. */
  customDownloadInfo: Readable<CustomDownloadInfo | null>;
  /** Which category the *currently active* download/picker belongs to. */
  currentCategory: Readable<ModelCategory>;
  /** Download progress 0-100 */
  progress: Readable<number>;
  /** Download speed string (e.g., "5.2 MB/s") */
  speed: Readable<string>;
  /** Time remaining string (e.g., "2:30") */
  timeRemaining: Readable<string>;
  /** Time elapsed string */
  timeElapsed: Readable<string>;
  /** Status message from backend */
  status: Readable<string>;
}

export interface ModelDownloadActions {
  /** Open the download modal for a category (defaults to transcription). */
  openModal: (category?: ModelCategory) => void;
  /** Close the download modal (download continues in background) */
  closeModal: () => void;
  /** Synchronously hand off to the shared dialog for a BYO custom download. */
  beginCustomDownload: (info: CustomDownloadInfo) => void;
  /** Setup Tauri event listeners - singleton, idempotent across calls. */
  setupListeners: (listen: TauriListen) => Promise<() => void>;
  /** Clear the paused-download card's metadata (eg after Delete-leftover). */
  clearCustomDownloadInfo: () => void;
  /** Trigger glow effect callback */
  onDownloadComplete?: () => void;
}

export type ModelDownloadEvents = ModelDownloadState & ModelDownloadActions;

function createModelDownloadEvents(): ModelDownloadEvents {
  const isModalOpen = writable(false);
  const isDownloadActive = writable(false);
  // True while the active download is a BYO custom-model import (as opposed
  // to a registry model) - lets callers like the mini pill route "restore"
  // clicks to ImportCustomModelModal instead of the registry picker dialog.
  const isCustomDownload = writable(false);
  const customDownloadInfo = writable<CustomDownloadInfo | null>(null);
  const currentCategory = writable<ModelCategory>('transcription');
  const progress = writable(0);
  const speed = writable('');
  const timeRemaining = writable('');
  const timeElapsed = writable('');
  const status = writable('');

  let onDownloadCompleteCallback: (() => void) | undefined;
  let listenersInstalled = false;

  function openModal(category: ModelCategory = 'transcription') {
    currentCategory.set(category);
    isModalOpen.set(true);
  }

  function closeModal() {
    isModalOpen.set(false);
  }

  /**
   * Hand off to the SAME shared download dialog for a BYO custom-model
   * import, exactly like `selectModel('custom')` in ModelDownloadProgress.svelte
   * hands off the other way (registry picker -> ImportCustomModelModal).
   * Called synchronously by ImportCustomModelModal right as a download
   * starts, before the backend's `model-download-start` event round-trips -
   * without this, there'd be a flash where neither dialog is visible between
   * the import modal closing and the event confirming the state change.
   * The event handler below sets the same fields again when it arrives;
   * that's a harmless no-op repeat, not a race - both writes agree.
   */
  function beginCustomDownload(info: CustomDownloadInfo) {
    currentCategory.set('transcription');
    isCustomDownload.set(true);
    customDownloadInfo.set(info);
    isDownloadActive.set(true);
    isModalOpen.set(true);
    progress.set(0);
  }

  function resetProgress() {
    progress.set(0);
    speed.set('');
    timeRemaining.set('');
    timeElapsed.set('');
    status.set('');
  }

  /** Pick the right active-model store for a category. */
  function storeForCategory(category: ModelCategory) {
    return category === 'transformation' ? transformModelStore : modelStore;
  }

  /** Look up category for a model id; default transcription if unknown. */
  function categoryForModel(modelId: string | undefined): ModelCategory {
    if (!modelId) return 'transcription';
    const def = getModelById(modelId);
    return def?.category ?? 'transcription';
  }

  async function setupListeners(listen: any): Promise<() => void> {
    // Singleton-style: ignore repeat calls. Returning a no-op cleanup keeps
    // the API stable for callers that always call cleanup on unmount.
    if (listenersInstalled) return () => {};
    listenersInstalled = true;

    const unlisteners: (() => void)[] = [];

    // Download started - derive category from the registry via model_id.
    unlisteners.push(
      await listen('model-download-start', (event: any) => {
        const payload = event.payload as { model_id?: string; model_dir_name?: string };
        const cat = categoryForModel(payload?.model_id);
        currentCategory.set(cat);
        // Custom downloads already got here via beginCustomDownload() (called
        // synchronously by ImportCustomModelModal for a flash-free handoff) -
        // this just re-confirms the same state. Same shared dialog either way.
        isModalOpen.set(true);
        isCustomDownload.set(payload?.model_id === 'custom');
        isDownloadActive.set(true);
        progress.set(0);
        // Backend knows the real on-disk directory name; merge it in for the
        // check/delete-partial-download commands (beginCustomDownload's
        // caller can't know this ahead of the backend resolving it).
        if (payload?.model_id === 'custom' && payload.model_dir_name) {
          customDownloadInfo.update(info =>
            info ? { ...info, modelDirName: payload.model_dir_name } : info
          );
        }
      })
    );

    // Download progress
    unlisteners.push(
      await listen('model-download-progress', (event: any) => {
        const payload = event.payload as {
          progress: number;
          speed?: string;
          timeRemaining?: string;
          timeElapsed?: string;
          status?: string;
        };

        progress.set(payload.progress);
        speed.set(payload.speed || '');
        timeRemaining.set(payload.timeRemaining || '');
        timeElapsed.set(payload.timeElapsed || '');
        status.set(payload.status || '');
      })
    );

    // Download cancelled
    unlisteners.push(
      await listen('model-download-cancelled', () => {
        isDownloadActive.set(false);
        isCustomDownload.set(false);
        resetProgress();
      })
    );

    // Download complete - route store update + preload to the right category.
    unlisteners.push(
      await listen('model-download-complete', async (event: any) => {
        const downloadedModelId = event.payload as string;
        const cat = categoryForModel(downloadedModelId);
        const store = storeForCategory(cat);

        // Don't close modal - let the component play its completion animation.
        isDownloadActive.set(false);
        isCustomDownload.set(false);
        customDownloadInfo.set(null);
        progress.set(100);
        speed.set('');
        timeRemaining.set('');
        status.set(translate('modelDownload.complete'));

        await store.selectModel(downloadedModelId);
        store.markModelDownloaded(downloadedModelId);

        // Clear cache + preload - only meaningful for transcription today;
        // transform corrector lazy-loads on first call (preload is no-op).
        try {
          await store.clearCache();
          await store.preloadModel();
        } catch (preloadErr) {
          console.error('Failed to preload new model:', preloadErr);
        }

        if (onDownloadCompleteCallback) {
          onDownloadCompleteCallback();
        }
      })
    );

    // Model now exists - refresh whichever category just changed.
    unlisteners.push(
      await listen('model-now-exists', async () => {
        await modelStore.refreshDownloadedModels();
        await transformModelStore.refreshDownloadedModels();
      })
    );

    // Download error
    unlisteners.push(
      await listen('model-download-error', () => {
        isDownloadActive.set(false);
        isCustomDownload.set(false);
        customDownloadInfo.set(null);
        // Don't close the modal - the dialog renders its own error state.
      })
    );

    return () => {
      unlisteners.forEach(unlisten => unlisten());
      listenersInstalled = false;
    };
  }

  return {
    isModalOpen,
    isDownloadActive: { subscribe: isDownloadActive.subscribe },
    isCustomDownload: { subscribe: isCustomDownload.subscribe },
    customDownloadInfo: { subscribe: customDownloadInfo.subscribe },
    currentCategory: { subscribe: currentCategory.subscribe },
    progress: { subscribe: progress.subscribe },
    speed: { subscribe: speed.subscribe },
    timeRemaining: { subscribe: timeRemaining.subscribe },
    timeElapsed: { subscribe: timeElapsed.subscribe },
    status: { subscribe: status.subscribe },
    openModal,
    closeModal,
    beginCustomDownload,
    clearCustomDownloadInfo: () => customDownloadInfo.set(null),
    setupListeners,
    set onDownloadComplete(callback: (() => void) | undefined) {
      onDownloadCompleteCallback = callback;
    },
  };
}

/**
 * Singleton instance shared across the app. `+page.svelte` calls
 * `setupListeners` once; `SettingsModal` calls `openModal(category)` to
 * surface the picker. Any consumer can read the stores reactively.
 */
export const modelDownloadEvents = createModelDownloadEvents();

/** Backward-compat factory - returns the singleton. */
export function useModelDownloadEvents(): ModelDownloadEvents {
  return modelDownloadEvents;
}
