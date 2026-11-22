// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable, get } from 'svelte/store';
import { mustInvoke } from '$lib/ipc/invoke';
import { getSettings, updateSetting } from '$lib/ipc/settings';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Settings } from '../types/settings';
import {
  DEFAULT_MODEL_ID,
  getModelById,
  getRecommendedModel,
  getAvailableModels,
} from '../types/models';
import { showModelAutoSwitchToast } from './modelAutoSwitchToast';
import { appLog, appWarn } from '$lib/utils/appLog';
import { translate } from './locale.svelte';

// ============================================================================
// Preload progress stores (C5 - exposed for UI hint rendering)
// ============================================================================

/** Current preload phase emitted by `model-preload-status` Rust events. */
export const preloadPhase = writable<'idle' | 'loading' | 'done' | 'failed'>('idle');

/** Elapsed seconds since preload started, updated via heartbeat events. */
export const preloadElapsedSec = writable<number>(0);

// JS-side safety net ONLY. The authoritative signal is Rust's
// `model-preload-status`/`model-preload-finished` events (preloadPhase store).
// This timer must sit ABOVE the Rust preload ceiling so Rust reports its own
// outcome first - otherwise the JS timer fires while Rust is still loading and
// the preload looks falsely resolved. The Rust ceiling is 60s (see
// commands/transcription.rs `PRELOAD_CEILING_SECS`); we add a 15s buffer.
// Single source of truth lives in Rust; keep these two in sync.
const PRELOAD_TIMEOUT_MS = 75_000;

interface PreloadStartedEvent {
  backend: string;
  model: string;
}

interface PreloadFinishedEvent {
  backend: string;
  model: string;
  duration_ms: number;
  ok: boolean;
  error?: string;
}

let preloadEventsListening = false;
let preloadStartedUnlisten: UnlistenFn | null = null;
let preloadFinishedUnlisten: UnlistenFn | null = null;
let preloadStatusUnlisten: UnlistenFn | null = null;

// Dedupe guard: if a preload invoke is already in flight (e.g. HMR reload
// kicked off a second call while the first is still running), return the
// existing Promise instead of stacking Rust invocations.
let inFlightPreload: Promise<void> | null = null;

// Store state interface
interface ModelStoreState {
  selectedModel: string;
  modelExists: boolean;
  downloadedModels: Record<string, boolean>;
  isInitialized: boolean;
  isChecking: boolean;
  isPreloading: boolean;
  preloadError: string | null;
}

// Create the writable store
function createModelStore() {
  const { subscribe, set, update } = writable<ModelStoreState>({
    selectedModel: DEFAULT_MODEL_ID, // Default to recommended model
    modelExists: false,
    downloadedModels: {},
    isInitialized: false,
    isChecking: false,
    isPreloading: false,
    preloadError: null,
  });

  // Initialize the store from Tauri settings
  async function initialize() {
    try {
      update(state => ({ ...state, isChecking: true }));

      // Get settings and ORT availability in parallel
      const [settings, ortAvailable] = await Promise.all([
        mustInvoke('get_settings') as Promise<Settings>,
        mustInvoke('check_ort_available') as Promise<boolean>,
      ]);

      // Check which models are downloaded
      const modelStatuses = (await mustInvoke('check_models_exist')) as [string, boolean][];
      const downloadedModels: Record<string, boolean> = {};

      if (modelStatuses && Array.isArray(modelStatuses)) {
        modelStatuses.forEach(([model, exists]) => {
          downloadedModels[model] = exists;
        });
      }

      // Check if selected model exists
      let selectedModel = settings.model || DEFAULT_MODEL_ID;
      let modelExists = (await mustInvoke('check_selected_model_exists')) as boolean;

      // Auto-select an available model if current selection doesn't exist
      if (!modelExists) {
        const downloadedModelIds = Object.entries(downloadedModels)
          .filter(([_, exists]) => exists)
          .map(([id]) => id);

        if (downloadedModelIds.length > 0) {
          // Prefer recommended model if downloaded
          const recommended = getRecommendedModel();
          if (recommended && downloadedModels[recommended.id]) {
            selectedModel = recommended.id;
            console.log('[ModelStore] Auto-selected recommended model:', selectedModel);
          } else {
            // Fallback: prioritize by quality, respecting ORT availability
            // Without this, old Macs would auto-select a downloaded ONNX model
            // that can't actually run (ORT requires macOS 13.3+)
            const availableModels = getAvailableModels(ortAvailable);
            const bestAvailable = availableModels.find(m => downloadedModels[m.id]);
            if (bestAvailable) {
              selectedModel = bestAvailable.id;
              console.log('[ModelStore] Auto-selected best available model:', selectedModel);
            } else {
              // Last resort: just use first downloaded model
              selectedModel = downloadedModelIds[0];
              console.log('[ModelStore] Auto-selected first available model:', selectedModel);
            }
          }
          modelExists = true;

          // Persist the auto-selection to settings
          try {
            await updateSetting('model', selectedModel);
          } catch (saveErr) {
            console.warn('[ModelStore] Failed to persist auto-selected model:', saveErr);
          }

          // Never switch models silently - the user may have deliberately
          // selected the (now unavailable) model and would otherwise only
          // notice via wrong transcriptions (eg Parakeet mangling Mandarin).
          const previousModel = settings.model || DEFAULT_MODEL_ID;
          if (previousModel !== selectedModel) {
            showModelAutoSwitchToast({ from: previousModel, to: selectedModel });
          }
        }
      }

      update(state => ({
        ...state,
        selectedModel,
        modelExists,
        downloadedModels,
        isInitialized: true,
        isChecking: false,
      }));

      console.log('[ModelStore] Initialized:', {
        selectedModel,
        modelExists,
        downloadedModels,
      });
    } catch (err) {
      console.error('[ModelStore] Failed to initialize:', err);
      update(state => ({ ...state, isChecking: false, isInitialized: true }));
    }
  }

  /**
   * Select a model and save to Tauri.
   *
   * @param opts.preload When true (default) the new model is swapped into
   *   memory immediately - correct for the model-picker UI. App init passes
   *   `false` because it runs its own preload *after* EP detection and auth;
   *   preloading here too raced that call and produced a bogus
   *   "preload did not complete" warning.
   */
  async function selectModel(modelId: string, opts?: { preload?: boolean }) {
    const shouldPreload = opts?.preload ?? true;
    try {
      // Skip only when the BACKEND already agrees. Store state alone is not
      // enough: it can drift from the backend, and skipping on a drifted store
      // silently drops the user's re-selection while the UI keeps showing the
      // model they picked. Partial patches make drift far less likely, but the
      // check is cheap and this failure mode was invisible to the user.
      const currentState = get(modelStore);
      appLog(`[ModelStore] selectModel requested: ${currentState.selectedModel} -> ${modelId}`);
      // 'custom' keeps the same id across Replace imports, so "already
      // selected" does NOT mean "already loaded" - always fall through to the
      // reload path so the freshly imported file actually gets swapped in.
      if (currentState.selectedModel === modelId && modelId !== 'custom') {
        const persistedModel = (await getSettings()).model;
        if (persistedModel === modelId) {
          appLog(`[ModelStore] already selected, skipping: ${modelId}`);
          return;
        }
        appWarn(
          `[ModelStore] store says '${modelId}' but backend says '${persistedModel}' - re-persisting`
        );
      }

      // Update store immediately for UI responsiveness
      update(state => ({ ...state, selectedModel: modelId }));

      // Save to Tauri settings (single-key patch - cannot revert other fields)
      await updateSetting('model', modelId);
      appLog(`[ModelStore] model persisted: ${modelId}`);

      // Check if the new model exists. Custom (BYO) models all collapse to
      // the single cache key 'custom' in downloadedModels, so that cache
      // can't distinguish between different imported models - it's stale
      // the moment the user picks a different custom model. Ask the backend
      // directly in that case; it re-reads settings.custom_model_path live.
      let modelExists: boolean;
      if (modelId === 'custom') {
        modelExists = (await mustInvoke('check_selected_model_exists')) as boolean;
      } else {
        const state = get(modelStore);
        modelExists = state.downloadedModels[modelId] || false;
      }

      update(state => ({ ...state, modelExists }));

      console.log('[ModelStore] Model selected:', modelId, 'exists:', modelExists);

      // Dispatch event for other components
      window.dispatchEvent(
        new CustomEvent('model-selected', {
          detail: { model: modelId, exists: modelExists },
        })
      );

      // Swap the in-memory model to match the new selection. Without this the
      // PREVIOUS model stays resident and the first transcription after a
      // switch pays the full cold load (~10s for Qwen3-ASR) while also holding
      // two models' worth of VRAM.
      if (modelExists) {
        if (shouldPreload) {
          appLog(`[ModelStore] swapping loaded model to ${modelId}`);
          markPreloadQueued();
          void reloadModel().catch(err => {
            appWarn(`[ModelStore] preload after switch failed: ${err}`);
            markPreloadAbandoned();
          });
        } else {
          // Caller preloads itself later. Still drop the stale model so the
          // previous selection isn't left resident holding VRAM.
          appLog(`[ModelStore] unloading stale model (caller will preload ${modelId})`);
          // Any dedupe promise now refers to the model we just unloaded.
          inFlightPreload = null;
          void mustInvoke('unload_model').catch(err => {
            appWarn(`[ModelStore] unload_model after switch failed: ${err}`);
          });
        }
      }
    } catch (err) {
      console.error('[ModelStore] Failed to select model:', err);
    }
  }

  // Check if currently selected model exists
  async function checkSelectedModelExists() {
    try {
      update(state => ({ ...state, isChecking: true }));
      const exists = (await mustInvoke('check_selected_model_exists')) as boolean;
      update(state => ({
        ...state,
        modelExists: exists,
        isChecking: false,
      }));
      return exists;
    } catch (err) {
      console.error('[ModelStore] Failed to check model:', err);
      update(state => ({ ...state, isChecking: false }));
      return false;
    }
  }

  // Refresh downloaded models status
  async function refreshDownloadedModels() {
    try {
      const modelStatuses = (await mustInvoke('check_models_exist')) as [string, boolean][];
      const downloadedModels: Record<string, boolean> = {};

      if (modelStatuses && Array.isArray(modelStatuses)) {
        modelStatuses.forEach(([model, exists]) => {
          downloadedModels[model] = exists;
        });
      }

      const state = get(modelStore);
      const modelExists = downloadedModels[state.selectedModel] || false;

      update(state => ({
        ...state,
        downloadedModels,
        modelExists,
      }));

      console.log('[ModelStore] Refreshed downloaded models:', downloadedModels);
    } catch (err) {
      console.error('[ModelStore] Failed to refresh models:', err);
    }
  }

  // Mark model as downloaded
  function markModelDownloaded(modelId: string) {
    update(state => {
      const downloadedModels = { ...state.downloadedModels, [modelId]: true };
      const modelExists = state.selectedModel === modelId ? true : state.modelExists;
      return { ...state, downloadedModels, modelExists };
    });
  }

  // Attach listeners for Rust-emitted preload events. Log-only for now; they
  // give future debugging a way to see whether the backend actually entered
  // `spawn_blocking` when a preload hang is reported. Idempotent.
  async function ensurePreloadEventListeners(): Promise<void> {
    if (preloadEventsListening) return;
    preloadEventsListening = true;
    try {
      preloadStartedUnlisten = await listen<PreloadStartedEvent>('model-preload-started', event => {
        console.log('[ModelStore] model-preload-started', event.payload);
        preloadPhase.set('loading');
        preloadElapsedSec.set(0);
      });
      preloadFinishedUnlisten = await listen<PreloadFinishedEvent>(
        'model-preload-finished',
        event => {
          const { ok, duration_ms, backend, model, error } = event.payload;
          const seconds = (duration_ms / 1000).toFixed(2);
          if (ok) {
            console.log(
              `%c Model preloaded in ${seconds}s (${duration_ms}ms, ${backend}, ${model})`,
              'color:#a3e635;font-weight:600'
            );
            preloadPhase.set('done');
            if (duration_ms > 15_000) {
              console.warn(
                `[ModelStore] Preload took ${seconds}s - this is unusually slow. ` +
                  `If you expected GPU acceleration, check Settings > GPU Acceleration.`
              );
            }
          } else {
            console.warn(
              `[ModelStore] Model preload FAILED after ${seconds}s (${backend}, ${model}): ${error ?? 'unknown'}`
            );
            preloadPhase.set('failed');
          }
        }
      );
      // Subscribe to fine-grained preload status events (loading/done/failed + heartbeat)
      preloadStatusUnlisten = await listen<{ phase: string; elapsed_ms: number; error?: string }>(
        'model-preload-status',
        event => {
          const { phase, elapsed_ms, error } = event.payload;
          const elapsedSec = Math.round(elapsed_ms / 1000);
          if (phase === 'loading') {
            preloadPhase.set('loading');
            preloadElapsedSec.set(elapsedSec);
          } else if (phase === 'done') {
            preloadPhase.set('done');
            preloadElapsedSec.set(elapsedSec);
          } else if (phase === 'failed') {
            preloadPhase.set('failed');
            preloadElapsedSec.set(elapsedSec);
            console.warn('[ModelStore] model-preload-status: failed -', error ?? 'unknown');
          }
        }
      );
    } catch (err) {
      // Listener registration failure is non-fatal.
      preloadEventsListening = false;
      console.warn('[ModelStore] Failed to register preload event listeners:', err);
    }
  }

  // Flip isPreloading=true the instant the app decides it WILL preload, before
  // the auth/EP wait window the actual preloadModel() awaits. Without this gate,
  // the record button stays clickable during that ~5s window and a record-click
  // triggers a lazy load that races the background preload. The serialise-
  // from_pretrained fix (b384fc0) prevents the deadlock that race used to cause,
  // but it still wastes the user's first transcribe on a cold-shader load.
  function markPreloadQueued(): void {
    update(state => ({ ...state, isPreloading: true, preloadError: null }));
    preloadPhase.set('loading');
  }

  // Counterpart for cases where the queued preload doesn't actually run
  // (auth never readied, EP detection threw, etc.). Without this the gate
  // from markPreloadQueued() would keep the record button disabled forever.
  function markPreloadAbandoned(): void {
    update(state => ({ ...state, isPreloading: false }));
    preloadPhase.set('idle');
  }

  // Preload model into GPU memory for instant transcriptions.
  // Wrapped in a timeout so a wedged backend can't freeze the UI forever.
  // Deduped: HMR reloads can trigger this multiple times while the Rust
  // side is still processing the first invoke - return the existing Promise.
  function preloadModel(): Promise<void> {
    if (inFlightPreload) {
      console.log('[ModelStore] Preload already in flight, returning existing promise');
      return inFlightPreload;
    }

    inFlightPreload = doPreload().finally(() => {
      inFlightPreload = null;
    });
    return inFlightPreload;
  }

  async function doPreload(): Promise<void> {
    await ensurePreloadEventListeners();
    update(state => ({ ...state, isPreloading: true, preloadError: null }));

    let timeoutHandle: ReturnType<typeof setTimeout> | undefined;
    const timeoutPromise = new Promise<never>((_, reject) => {
      timeoutHandle = setTimeout(() => {
        reject(new Error('PRELOAD_TIMEOUT'));
      }, PRELOAD_TIMEOUT_MS);
    });

    try {
      console.log('[ModelStore] Preloading model...');
      await Promise.race([mustInvoke('preload_whisper_model'), timeoutPromise]);
      console.log('[ModelStore] Model preloaded successfully');
      update(state => ({ ...state, isPreloading: false }));
    } catch (err) {
      const isTimeout = err instanceof Error && err.message === 'PRELOAD_TIMEOUT';
      const errorMsg = isTimeout
        ? translate('modelStore.preloadTimeout')
        : err instanceof Error
          ? err.message
          : String(err);
      if (isTimeout) {
        console.warn(
          `[ModelStore] Preload timed out after ${PRELOAD_TIMEOUT_MS}ms - will load on first use`
        );
      } else {
        console.error('[ModelStore] Preload failed:', errorMsg);
      }
      // A JS timeout is NOT success. We set preloadError and leave preloadPhase
      // untouched (Rust remains authoritative and will emit `done`/`failed`).
      // The promise resolves on timeout to keep "abandon, fall back to lazy
      // load" semantics, so callers MUST gate any success messaging on
      // preloadPhase === 'done' (or !preloadError) rather than on resolution.
      update(state => ({
        ...state,
        isPreloading: false,
        preloadError: errorMsg,
      }));
      if (!isTimeout) throw err;
    } finally {
      if (timeoutHandle !== undefined) clearTimeout(timeoutHandle);
    }
  }

  // Clear model cache (call before switching models)
  async function clearCache(): Promise<void> {
    try {
      console.log('[ModelStore] Clearing model cache...');
      await mustInvoke('clear_whisper_model_cache');
      console.log('[ModelStore] Model cache cleared');
    } catch (err) {
      console.error('[ModelStore] Failed to clear cache:', err);
    }
  }

  /**
   * Unload the currently-loaded model from memory, then preload it again.
   * Useful for profiling cold-load times and for debugging preload hangs.
   *
   * Clears both Candle (Whisper) + ONNX (Parakeet) caches server-side and
   * clears Rust's LOADED_MODEL record, then triggers a fresh preload.
   * Normal `model-preload-started` / `model-preload-finished` events fire,
   * so the spinner and timing log work exactly like first-boot.
   */
  async function reloadModel(): Promise<void> {
    console.log('[ModelStore] 🔄 Reloading model (unload → preload)…');
    try {
      await mustInvoke('unload_model');
    } catch (err) {
      console.error('[ModelStore] unload_model failed:', err);
      // Don't return - still try to preload so the UI doesn't get stuck.
    }
    // Clear frontend-side dedupe so preloadModel() runs a fresh doPreload().
    inFlightPreload = null;
    await preloadModel();
  }

  return {
    subscribe,
    initialize,
    selectModel,
    checkSelectedModelExists,
    refreshDownloadedModels,
    markModelDownloaded,
    markPreloadQueued,
    markPreloadAbandoned,
    preloadModel,
    reloadModel,
    clearCache,
    // Helper getters
    getSelectedModel: () => get(modelStore).selectedModel,
    getModelExists: () => get(modelStore).modelExists,
    isModelDownloaded: (modelId: string) => get(modelStore).downloadedModels[modelId] || false,
  };
}

// Create and export the singleton store
export const modelStore = createModelStore();

// Note: modelStore.initialize() is called explicitly by useAppInit.ts
// Do NOT auto-init here - it races with the explicit call and doubles IPC load
