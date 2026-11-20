// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Windows CUDA plugin lifecycle composable.
 *
 * Owns the install / cancel / uninstall flow and the three Tauri events
 * (`ort-plugin-progress`, `ort-plugin-complete`, `ort-plugin-error`) that
 * drive the GPU Acceleration help modal. Caller subscribes to the
 * returned stores and dispatches actions; teardown drops listeners and
 * resets state.
 *
 * Extracted from GpuAccelerationSection.svelte (1297-line god file).
 */

import { writable, type Readable } from 'svelte/store';
import { listen } from '@tauri-apps/api/event';
import { isTauriAvailable, mustInvoke, safeInvoke } from '$lib/ipc/invoke';
import { devError } from '$lib/utils/devLog';

export interface PluginStatus {
  installed: boolean;
  version: string | null;
  active_in_current_session: boolean;
  compatible: boolean;
  plugin_dir: string;
}

export interface PluginDownload {
  active: boolean;
  progress: number;
  speed: string;
  remaining: string;
  stage: string;
  status: string;
}

const INITIAL_DOWNLOAD: PluginDownload = {
  active: false,
  progress: 0,
  speed: '',
  remaining: '',
  stage: '',
  status: '',
};

interface PluginProgressPayload {
  progress?: number;
  speed?: string;
  timeRemaining?: string;
  stage?: string;
  status?: string;
}

export interface CudaPluginState {
  status: Readable<PluginStatus | null>;
  download: Readable<PluginDownload>;
  error: Readable<string | null>;
}

export interface CudaPluginActions {
  /** Refresh plugin status from the backend. */
  refresh: () => Promise<void>;
  /** Begin downloading + installing the CUDA plugin. */
  install: () => Promise<void>;
  /** Cancel an in-flight install. */
  cancel: () => Promise<void>;
  /** Remove the installed plugin. */
  uninstall: () => Promise<void>;
  /**
   * Subscribe to the `ort-plugin-*` Tauri events. Idempotent: safe to
   * call multiple times; state is overwritten by the events.
   */
  attachListeners: () => Promise<void>;
  /** Drop event listeners and clear local state. */
  cleanup: () => void;
  /** Clear the current error message (used when closing the help modal). */
  clearError: () => void;
}

export type CudaPlugin = CudaPluginState & CudaPluginActions;

export function useCudaPlugin(): CudaPlugin {
  const status = writable<PluginStatus | null>(null);
  const download = writable<PluginDownload>({ ...INITIAL_DOWNLOAD });
  const error = writable<string | null>(null);

  const unsubs: Array<() => void> = [];

  async function refresh(): Promise<void> {
    if (!isTauriAvailable()) return;
    try {
      status.set(await mustInvoke<PluginStatus>('get_cuda_plugin_status'));
    } catch (err) {
      devError('[CudaPlugin] get_cuda_plugin_status failed:', err);
    }
  }

  async function install(): Promise<void> {
    if (!isTauriAvailable()) return;
    error.set(null);
    download.set({
      active: true,
      progress: 0,
      speed: '',
      remaining: '',
      stage: 'download',
      status: 'Starting',
    });
    try {
      await mustInvoke<void>('install_cuda_plugin');
      // install resolves before the complete event; events drive final state
    } catch (err) {
      error.set(String(err));
      download.update(d => ({ ...d, active: false }));
    }
  }

  async function cancel(): Promise<void> {
    await safeInvoke<void>('cancel_cuda_plugin_install');
  }

  async function uninstall(): Promise<void> {
    if (!isTauriAvailable()) return;
    try {
      await mustInvoke<void>('uninstall_cuda_plugin');
      await refresh();
    } catch (err) {
      error.set(String(err));
    }
  }

  async function attachListeners(): Promise<void> {
    if (!isTauriAvailable()) return;
    unsubs.push(
      await listen<PluginProgressPayload>('ort-plugin-progress', e => {
        const p = e.payload ?? {};
        download.update(current => ({
          active: true,
          progress: typeof p.progress === 'number' ? p.progress : current.progress,
          speed: p.speed ?? '',
          remaining: p.timeRemaining ?? '',
          stage: p.stage ?? '',
          status: p.status ?? '',
        }));
      }),
      await listen<unknown>('ort-plugin-complete', async () => {
        download.set({
          active: false,
          progress: 100,
          speed: '',
          remaining: '',
          stage: 'done',
          status: 'Done',
        });
        await refresh();
      }),
      await listen<string>('ort-plugin-error', e => {
        error.set(String(e.payload ?? 'Unknown error'));
        download.update(d => ({ ...d, active: false }));
      })
    );
  }

  function cleanup(): void {
    for (const off of unsubs) off();
    unsubs.length = 0;
  }

  return {
    status: { subscribe: status.subscribe },
    download: { subscribe: download.subscribe },
    error: { subscribe: error.subscribe },
    refresh,
    install,
    cancel,
    uninstall,
    attachListeners,
    cleanup,
    clearError: () => error.set(null),
  };
}
