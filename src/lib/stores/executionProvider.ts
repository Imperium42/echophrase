// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { mustInvoke } from '$lib/ipc/invoke';

const EP_DETECT_TIMEOUT_MS = 8_000;

interface ExecutionProviderState {
  active: string;
  preferred: string | null;
  isCpuFallback: boolean;
  isLoading: boolean;
}

let inFlightInit: Promise<void> | null = null;

function createExecutionProviderStore() {
  const { subscribe, set, update } = writable<ExecutionProviderState>({
    active: 'cpu',
    preferred: null,
    isCpuFallback: true,
    isLoading: true,
  });

  function initialize(): Promise<void> {
    if (inFlightInit) {
      console.log('[EP Store] Init already in flight, returning existing promise');
      return inFlightInit;
    }
    inFlightInit = doInitialize().finally(() => {
      inFlightInit = null;
    });
    return inFlightInit;
  }

  async function doInitialize(): Promise<void> {
    const t0 = performance.now();
    let timeoutHandle: ReturnType<typeof setTimeout> | undefined;
    const timeoutPromise = new Promise<never>((_, reject) => {
      timeoutHandle = setTimeout(
        () => reject(new Error('EP_DETECT_TIMEOUT')),
        EP_DETECT_TIMEOUT_MS
      );
    });

    try {
      console.log('[EP Store] Detecting GPU...');
      const ep = await Promise.race([
        invoke<{ active: string; preferred: string | null; is_cpu_fallback: boolean }>(
          'get_execution_provider'
        ),
        timeoutPromise,
      ]);
      const elapsed = performance.now() - t0;
      console.log(
        `[EP Store] Detected: ${ep.active} (CPU fallback: ${ep.is_cpu_fallback}, ${elapsed.toFixed(0)}ms)`
      );
      set({
        active: ep.active,
        preferred: ep.preferred,
        isCpuFallback: ep.is_cpu_fallback,
        isLoading: false,
      });
    } catch (err) {
      const isTimeout = err instanceof Error && err.message === 'EP_DETECT_TIMEOUT';
      if (isTimeout) {
        console.error(
          `[EP Store] GPU detection timed out after ${EP_DETECT_TIMEOUT_MS}ms - defaulting to CPU`
        );
      } else {
        console.error('[EP Store] Failed to get execution provider:', err);
      }
      update(s => ({ ...s, isLoading: false }));
    } finally {
      if (timeoutHandle !== undefined) clearTimeout(timeoutHandle);
    }
  }

  async function setProvider(provider: string) {
    const normalizedProvider = provider.toLowerCase();
    try {
      await mustInvoke<void>('set_preferred_execution_provider', { provider: normalizedProvider });
      update(s => ({
        ...s,
        active: normalizedProvider,
        preferred: normalizedProvider,
        isCpuFallback: normalizedProvider === 'cpu',
      }));
    } catch (err) {
      console.error('[EP Store] Failed to set execution provider:', err);
      throw err;
    }
  }

  async function refresh() {
    await doInitialize();
  }

  return {
    subscribe,
    initialize,
    setProvider,
    refresh,
  };
}

export const executionProviderStore = createExecutionProviderStore();
