// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * App initialization composable
 * Orchestrates all onMount initialization logic into a single function
 */

import { writable, type Readable, get } from 'svelte/store';
import { modelStore, preloadPhase } from '$lib/stores/modelStore';
import { appLog, appWarn } from '$lib/utils/appLog';
import { loadGpuContext, type GpuInfo, type OsInfo } from '$lib/utils/GpuUtil';
import { initPlatformCache, platformCache } from '$lib/stores/platformCache';
import { maybeAutoDetectModel } from '$lib/utils/modelAutoDetect';
import { getModelById } from '$lib/types/models';
import { devError } from '$lib/utils/devLog';
import { executionProviderStore } from '$lib/stores/executionProvider';
import type { HotkeyModeOptions } from './useHotkeys';

let tauriWarningsPatched = false;
function patchTauriDevWarnings(): void {
  if (tauriWarningsPatched) return;
  tauriWarningsPatched = true;

  const originalWarn = console.warn;
  console.warn = (...args: unknown[]) => {
    const msg = typeof args[0] === 'string' ? args[0] : '';
    if (msg.includes('IPC custom protocol failed')) {
      originalWarn(
        ...args,
        '(expected on WSL - postMessage fallback works fine, no action needed)'
      );
      return;
    }
    if (msg.includes("Couldn't find callback id")) {
      originalWarn(...args, '(dev-mode only: HMR reload orphaned an in-flight Rust invoke)');
      return;
    }
    originalWarn(...args);
  };
}

export interface AppInitState {
  /** Whether the app is still initializing */
  isInitializing: Readable<boolean>;
  /** App version string */
  appVersion: Readable<string>;
  /** GPU info */
  gpuInfo: Readable<GpuInfo>;
  /** OS info (platform, WSL detection) */
  osInfo: Readable<OsInfo>;
  /** Tauri invoke function (null in web mode) */
  invoke: typeof import('@tauri-apps/api/core').invoke | null;
  /** Tauri listen function (null in web mode) */
  listen: typeof import('@tauri-apps/api/event').listen | null;
}

export interface InitCallbacks {
  /** Called when download events should be set up */
  setupDownloadListeners: (
    listen: typeof import('@tauri-apps/api/event').listen
  ) => Promise<() => void>;
  /** Called when transcription segment is received */
  onTranscriptionSegment: (
    text: string,
    isFinal: boolean,
    transformInfo?: {
      isTransformed?: boolean;
      isOriginalBeforeTransform?: boolean;
      originalText?: string;
      isSilence?: boolean;
    }
  ) => void;
  /** Called when hotkeys should toggle recording */
  onHotkeyToggle: () => void;
  /** Called to get processing state for hotkey disable */
  isHotkeyDisabled: () => boolean;
  /** Hotkey press/release mode (hybrid/toggle/hold); omitted = toggle only */
  hotkeyMode?: HotkeyModeOptions;
  /** Called when model download completes (for glow effect) */
  onDownloadComplete: () => void;
  /** Load Tauri settings */
  loadTauriSettings: () => Promise<void>;
}

export interface AppInitActions {
  /** Initialize the app - returns cleanup function */
  init: (callbacks: InitCallbacks) => Promise<() => void>;
}

export type AppInit = AppInitState & AppInitActions;

const defaultGpuInfo: GpuInfo = { name: null, vramGb: null, vramMb: null, displayString: null };
const defaultOsInfo: OsInfo = {
  os: 'Unknown',
  isWsl: false,
  displayString: 'Unknown',
  cpuArch: 'unknown',
  linuxDistro: null,
  linuxDistroVersion: null,
};

/**
 * Creates the app initialization controller
 *
 * @example
 * ```svelte
 * <script>
 *   const appInit = useAppInit();
 *
 *   onMount(async () => {
 *     const cleanup = await appInit.init({
 *       setupDownloadListeners: ...,
 *       onTranscriptionSegment: ...,
 *       onHotkeyToggle: toggleRecording,
 *       isHotkeyDisabled: () => $isProcessing,
 *       onDownloadComplete: triggerGlow,
 *       loadTauriSettings: loadSettings,
 *     });
 *     return cleanup;
 *   });
 * </script>
 * ```
 */
export function useAppInit(): AppInit {
  const isInitializing = writable(true);
  const appVersion = writable('');
  const gpuInfo = writable<GpuInfo>(defaultGpuInfo);
  const osInfo = writable<OsInfo>(defaultOsInfo);

  let invoke: typeof import('@tauri-apps/api/core').invoke | null = null;
  let listen: typeof import('@tauri-apps/api/event').listen | null = null;

  // Intercept known Tauri dev-mode warnings and append context so they
  // don't look like real bugs in the console.
  patchTauriDevWarnings();

  async function init(callbacks: InitCallbacks): Promise<() => void> {
    const cleanupFns: (() => void)[] = [];
    const t0 = performance.now();
    const mark = (label: string) =>
      console.log(`[AppInit] ${label}: ${(performance.now() - t0).toFixed(0)}ms`);

    // Check for Tauri context
    if (typeof window.__TAURI_INTERNALS__ !== 'undefined') {
      try {
        const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
        const { listen: tauriListen } = await import('@tauri-apps/api/event');
        mark('Tauri imports');

        invoke = tauriInvoke;
        listen = tauriListen;

        // Parallel batch 1: IPC calls that don't depend on each other
        await Promise.all([
          // Reset stale recording state
          tauriInvoke('reset_recording_state').catch(err =>
            console.warn('[AppInit] Failed to reset recording state:', err)
          ),
          // Load settings from Tauri/YAML backend
          callbacks
            .loadTauriSettings()
            .then(() => console.log('Settings loaded from YAML'))
            .catch(err => console.warn('Failed to load settings:', err)),
          // Initialize locale
          import('$lib/stores/locale.svelte')
            .then(({ initLocale }) => initLocale())
            .then(locale => console.log('[AppInit] Locale initialized:', locale))
            .catch(err => console.warn('[AppInit] Failed to initialize locale:', err)),
        ]);
        mark('Batch 1 (settings, locale, reset)');

        // Load app version (fire-and-forget)
        import('@tauri-apps/api/app')
          .then(({ getVersion }) => getVersion())
          .then(v => appVersion.set(v))
          .catch(err => console.warn('Failed to get app version:', err));

        // GPU EP selection: ort 2.0.0-rc.12's with_auto_device(MaxPerformance)
        // handles this natively at session creation time. No app-level kick needed.

        // Deferred: GPU/OS info (seeds platform cache)
        loadGpuContext()
          .then(context => {
            gpuInfo.set(context.gpuInfo);
            osInfo.set(context.osInfo);
            initPlatformCache(
              context.osInfo.os,
              context.osInfo.isWsl,
              context.osInfo.cpuArch,
              context.osInfo.linuxDistro,
              context.osInfo.linuxDistroVersion
            );
          })
          .catch(err => console.warn('[AppInit] GPU context load failed:', err));

        // Parallel batch 2: event listeners (all independent)
        const [
          cleanupDownloadListeners,
          unlistenTranscriptionSegment,
          unlistenSettingsChanged,
          unlistenTrayToggle,
          hotkeyResult,
        ] = await Promise.all([
          // Download event listeners
          callbacks.setupDownloadListeners(tauriListen),
          // Transcription segment listener
          tauriListen<{ text: string; is_final: boolean }>('transcription-segment', event => {
            const { text, is_final } = event.payload;
            callbacks.onTranscriptionSegment(text, is_final, undefined);
          }),
          // Backend settings changed (any writer: a patch, auto-detection, the
          // tray). The backend is authoritative - resync so the UI can never
          // display a model the backend is not actually dispatching to.
          tauriListen('settings-changed', () => {
            import('$lib/composables/useTauriSettings')
              .then(({ refreshSettingsFromBackend }) => refreshSettingsFromBackend())
              .catch(err => console.warn('[AppInit] settings resync failed:', err));
          }),
          // Tray icon toggle-recording: fired when user clicks the tray context menu
          // item whose label flips between "Start Recording" and "Stop Recording".
          // Reuses the same toggle callback as the global hotkey.
          tauriListen('tray-toggle-recording', () => {
            if (!callbacks.isHotkeyDisabled()) {
              callbacks.onHotkeyToggle();
            }
          }),
          // Hotkeys
          import('./useHotkeys').then(async ({ useHotkeys }) => {
            const hotkeys = useHotkeys();
            const unsub = await hotkeys.initializeHotkeys(
              callbacks.onHotkeyToggle,
              callbacks.isHotkeyDisabled,
              callbacks.hotkeyMode
            );
            return { unsub, cleanup: hotkeys.cleanup };
          }),
        ]);

        mark('Batch 2 (listeners, hotkeys)');
        cleanupFns.push(
          cleanupDownloadListeners,
          unlistenTranscriptionSegment,
          unlistenSettingsChanged,
          unlistenTrayToggle
        );
        if (typeof hotkeyResult.unsub === 'function') cleanupFns.push(hotkeyResult.unsub);
        cleanupFns.push(hotkeyResult.cleanup);

        // JS-side transcription preview (sync, no IPC)
        const handlePreview = (e: Event) => {
          const detail = (e as CustomEvent).detail;
          const {
            text,
            is_final,
            isTransformed,
            isOriginalBeforeTransform,
            originalText,
            isSilence,
          } = detail;
          callbacks.onTranscriptionSegment(text, is_final, {
            isTransformed,
            isOriginalBeforeTransform,
            originalText,
            isSilence,
          });
        };
        window.addEventListener('transcription-preview', handlePreview);
        cleanupFns.push(() => window.removeEventListener('transcription-preview', handlePreview));
      } catch (error) {
        console.log('Failed to load Tauri APIs:', error);
      }
    } else {
      console.log('Running in web mode - Tauri APIs not available');
    }

    // App initialization complete - UI can render now
    mark('UI unblocked');
    isInitializing.set(false);

    // Show main window + close splash (window starts hidden, splash shows spinner)
    if (invoke) {
      // Failure here means the window stays hidden behind the splash - log it loudly
      // in dev so we notice; in prod, surfacing it would be too late to recover anyway.
      invoke('show_main_window').catch((e: unknown) =>
        devError('[AppInit] show_main_window failed:', e)
      );
    }

    // EP detection fires first (non-blocking) so it grabs the ORT lock before
    // the model preload does. The preload chains after EP via epReady below.
    // NOT awaited here - that would freeze the entire UI.
    const epReady = executionProviderStore
      .initialize()
      .then(() => mark('EP store'))
      .catch(err => console.error('[AppInit] EP store failed:', err));

    // Initialize model store (skip on unsupported Macs - no usable models)
    if (invoke && !platformCache.isUnsupportedMac) {
      try {
        await modelStore.initialize();
        console.log('Model store initialized:', {
          selectedModel: get(modelStore).selectedModel,
          modelExists: get(modelStore).modelExists,
          downloadedModels: get(modelStore).downloadedModels,
        });

        // Auto-detect optimal model on first launch
        try {
          const autoDetectResult = await maybeAutoDetectModel();
          if (autoDetectResult) {
            console.log('Model auto-detected:', autoDetectResult);
            // preload: false - the EP-gated block below is the single
            // preload caller during init. Letting selectModel() fire its own
            // un-awaited preload raced that call and bypassed the ordering
            // requirement documented there (EP must finish first).
            await modelStore.selectModel(autoDetectResult.model_id, { preload: false });
            const modelDef = getModelById(autoDetectResult.model_id);
            console.log(
              '[AutoDetect] Selected model:',
              modelDef?.name || autoDetectResult.model_id
            );
          }
        } catch (autoDetectErr) {
          console.warn('Model auto-detection failed:', autoDetectErr);
        }

        // Preload model in background once EP detection has finished (EP must
        // finish first so ORT's global lock is free).
        if (get(modelStore).modelExists) {
          // Pre-emptively gate the record button from the moment we decide to
          // preload. Otherwise the button stays clickable during the EP
          // wait and a record-click triggers a lazy load that races the
          // background preload. See markPreloadQueued() doc in modelStore.ts.
          modelStore.markPreloadQueued();

          epReady
            .then(() => {
              console.log('Preloading model in background...');
              return modelStore.preloadModel().then(() => {
                // preloadModel() resolves even on a JS-side timeout (abandon ->
                // lazy load), so resolution alone does NOT mean success. Rust's
                // preloadPhase is authoritative - only claim success on 'done'.
                // appLog/appWarn (not console.*) so the outcome lands in the
                // Rust log alongside the preload lines it describes - a plain
                // console.warn here is invisible in dev.log, which is how the
                // double-preload race stayed hidden.
                if (get(preloadPhase) === 'done') {
                  appLog('[AppInit] Model preloaded - transcriptions will be instant');
                } else {
                  appWarn(
                    `[AppInit] Preload did not complete (phase=${get(preloadPhase)}) - ` +
                      `model will load on first transcribe`
                  );
                }
              });
            })
            .catch(preloadErr => {
              console.error('Model preload failed (will load on first use):', preloadErr);
              modelStore.markPreloadAbandoned();
            });
        }
      } catch (error) {
        console.error('Failed to initialize model store:', error);
      }
    } else if (platformCache.isUnsupportedMac) {
      console.log('[AppInit] Skipping model initialization - unsupported Intel Mac');
    }

    // Check for updates (silent - don't bother user if up-to-date).
    // Skipped in dev: the check runs against the production CDN and compares
    // tauri.conf.json5's version, so any dev tree older than the latest
    // release shows a meaningless "update available" banner (installing it
    // would overwrite the dev build with the production installer). Manual
    // checks from Settings > About still work in dev.
    // Also skipped for package-manager installs (Scoop/winget/brew): the
    // manager owns the installed version, so self-updating would desync it.
    if (!import.meta.env.DEV && typeof window.__TAURI_INTERNALS__ !== 'undefined') {
      import('$lib/utils/managedInstall')
        .then(({ isManagedInstall }) => isManagedInstall())
        .catch(() => false)
        .then(managed => {
          if (managed) {
            console.log('[Updater] Managed install detected; skipping background update check.');
            return;
          }
          return import('$lib/stores/updater')
            .then(({ initUpdater, checkForUpdates }) => {
              initUpdater().then(() => {
                checkForUpdates(true).catch(err => {
                  console.warn('[Updater] Background check failed:', err);
                });
              });
            })
            .catch(() => {
              // Updater not available (e.g., web mode or missing plugin)
            });
        });
    }

    // Return combined cleanup function
    return () => {
      cleanupFns.forEach(fn => {
        try {
          fn();
        } catch (e) {
          console.warn('Cleanup error:', e);
        }
      });
    };
  }

  return {
    isInitializing: { subscribe: isInitializing.subscribe },
    appVersion: { subscribe: appVersion.subscribe },
    gpuInfo: { subscribe: gpuInfo.subscribe },
    osInfo: { subscribe: osInfo.subscribe },
    get invoke() {
      return invoke;
    },
    get listen() {
      return listen;
    },
    init,
  };
}
