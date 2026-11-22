// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Transform Model Store
 *
 * Parallel to `modelStore`, but tracks the active *transformation* model
 * (settings.transform_model) instead of the transcription model
 * (settings.model). Both stores share the same downloaded-models map
 * because `check_models_exist` returns every registered model.
 *
 * Shape mirrors modelStore so `ModelDownloadProgress.svelte` can take
 * either as a `store` prop without branching.
 */

import { writable, get } from 'svelte/store';
import { mustInvoke } from '$lib/ipc/invoke';
import { getSettings, updateSetting } from '$lib/ipc/settings';
import type { Settings } from '../types/settings';
import { getRecommendedModel, getAvailableModels } from '../types/models';

const DEFAULT_TRANSFORM_MODEL_ID = 't5-mini-grammar';

interface TransformModelStoreState {
  selectedModel: string;
  modelExists: boolean;
  downloadedModels: Record<string, boolean>;
  isInitialized: boolean;
  isChecking: boolean;
  // Preload state is unused for transform models today (the corrector lazy-loads
  // on first call), but keep the keys present so the same component can read
  // them without crashing.
  isPreloading: boolean;
  preloadError: string | null;
}

function createTransformModelStore() {
  const { subscribe, update } = writable<TransformModelStoreState>({
    selectedModel: DEFAULT_TRANSFORM_MODEL_ID,
    modelExists: true, // bundled tier is always present
    downloadedModels: {},
    isInitialized: false,
    isChecking: false,
    isPreloading: false,
    preloadError: null,
  });

  async function initialize() {
    try {
      update(state => ({ ...state, isChecking: true }));

      const settings = (await mustInvoke('get_settings')) as Settings;

      // Reuse the shared downloaded-models check; both stores want it.
      const modelStatuses = (await mustInvoke('check_models_exist')) as [string, boolean][];
      const downloadedModels: Record<string, boolean> = {};
      if (Array.isArray(modelStatuses)) {
        modelStatuses.forEach(([id, exists]) => {
          downloadedModels[id] = exists;
        });
      }

      let selectedModel = settings.transform_model || DEFAULT_TRANSFORM_MODEL_ID;
      const transformModels = getAvailableModels(true, 'transformation');
      const isKnown = transformModels.some(m => m.id === selectedModel);
      if (!isKnown) {
        const recommended = getRecommendedModel('transformation');
        selectedModel = recommended?.id ?? DEFAULT_TRANSFORM_MODEL_ID;
      }

      const modelExists = !!downloadedModels[selectedModel];

      update(state => ({
        ...state,
        selectedModel,
        modelExists,
        downloadedModels,
        isInitialized: true,
        isChecking: false,
      }));

      console.log('[TransformModelStore] Initialized:', { selectedModel, modelExists });
    } catch (err) {
      console.error('[TransformModelStore] Failed to initialize:', err);
      update(state => ({ ...state, isChecking: false, isInitialized: true }));
    }
  }

  async function selectModel(modelId: string) {
    try {
      const current = get(transformModelStore);
      // Same disk-verified skip-guard as modelStore.selectModel: store state
      // alone can desync from settings.yaml and silently drop the write.
      if (current.selectedModel === modelId && (await getSettings()).transform_model === modelId) {
        return;
      }

      update(state => ({ ...state, selectedModel: modelId }));

      await updateSetting('transform_model', modelId);

      const exists = !!current.downloadedModels[modelId];
      update(state => ({ ...state, modelExists: exists }));

      // Drop the cached corrector so the next correct() reloads from the new path.
      try {
        await mustInvoke('unload_transform_model');
      } catch (err) {
        console.warn('[TransformModelStore] unload_transform_model failed:', err);
      }

      console.log('[TransformModelStore] Selected:', modelId, 'exists:', exists);
    } catch (err) {
      console.error('[TransformModelStore] Failed to select model:', err);
    }
  }

  async function refreshDownloadedModels() {
    try {
      const modelStatuses = (await mustInvoke('check_models_exist')) as [string, boolean][];
      const downloadedModels: Record<string, boolean> = {};
      if (Array.isArray(modelStatuses)) {
        modelStatuses.forEach(([id, exists]) => {
          downloadedModels[id] = exists;
        });
      }
      const state = get(transformModelStore);
      const exists = !!downloadedModels[state.selectedModel];
      update(s => ({ ...s, downloadedModels, modelExists: exists }));
    } catch (err) {
      console.error('[TransformModelStore] Failed to refresh:', err);
    }
  }

  function markModelDownloaded(modelId: string) {
    update(state => {
      const downloadedModels = { ...state.downloadedModels, [modelId]: true };
      const modelExists = state.selectedModel === modelId ? true : state.modelExists;
      return { ...state, downloadedModels, modelExists };
    });
  }

  // No-op: transform corrector lazy-loads on first call. Keep API parity.
  async function preloadModel(): Promise<void> {
    // Intentionally empty.
  }

  // Type-parity twins of modelStore.markPreloadQueued / markPreloadAbandoned.
  // The transform corrector doesn't background-preload, so these are no-ops -
  // but consumers that take either store via a typeof modelStore prop need
  // the method names to exist on this interface too.
  function markPreloadQueued(): void {
    // Intentionally empty.
  }

  function markPreloadAbandoned(): void {
    // Intentionally empty.
  }

  async function clearCache(): Promise<void> {
    try {
      await mustInvoke('unload_transform_model');
    } catch (err) {
      console.warn('[TransformModelStore] unload_transform_model failed:', err);
    }
  }

  /**
   * Type-parity twin of modelStore.reloadModel - clears the in-memory
   * transform model then re-preloads it. Kept here so consumers that take
   * either store via a typeof modelStore prop don't fail svelte-check.
   */
  async function reloadModel(): Promise<void> {
    await clearCache();
    await preloadModel();
  }

  async function checkSelectedModelExists(): Promise<boolean> {
    const state = get(transformModelStore);
    return state.modelExists;
  }

  return {
    subscribe,
    initialize,
    selectModel,
    refreshDownloadedModels,
    markModelDownloaded,
    markPreloadQueued,
    markPreloadAbandoned,
    preloadModel,
    reloadModel,
    clearCache,
    checkSelectedModelExists,
    getSelectedModel: () => get(transformModelStore).selectedModel,
    getModelExists: () => get(transformModelStore).modelExists,
    isModelDownloaded: (modelId: string) =>
      get(transformModelStore).downloadedModels[modelId] || false,
  };
}

export const transformModelStore = createTransformModelStore();
