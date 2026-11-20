// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Debug logs service
 * Fetches and manages backend debug logs from Tauri
 */

import { writable, type Readable } from 'svelte/store';
import { mustInvoke, isTauriAvailable } from '$lib/ipc/invoke';

export interface DebugLogsState {
  /** Array of log entries (newest first) */
  logs: Readable<string[]>;
  /** Whether logs are currently being fetched */
  isRefreshing: Readable<boolean>;
}

export interface DebugLogsActions {
  /** Fetch logs from the backend */
  fetch: () => Promise<void>;
  /** Clear all logs */
  clear: () => void;
}

export type DebugLogsService = DebugLogsState & DebugLogsActions;

/**
 * Creates the debug logs service
 *
 * @example
 * ```svelte
 * <script>
 *   const debugLogs = createDebugLogsService();
 *   onMount(() => debugLogs.fetch(invoke));
 * </script>
 *
 * {#each $debugLogs.logs as log}
 *   <div>{log}</div>
 * {/each}
 * ```
 */
export function createDebugLogsService(): DebugLogsService {
  const logs = writable<string[]>([]);
  const isRefreshing = writable(false);

  async function fetch(): Promise<void> {
    isRefreshing.set(true);
    try {
      if (isTauriAvailable()) {
        console.log('Fetching debug logs...');
        const rawLogs = await mustInvoke<string[]>('get_debug_logs');
        // Reverse the logs so latest appears at the top
        logs.set((rawLogs ?? []).reverse());
        console.log('Debug logs fetched:', rawLogs?.length ?? 0, 'entries');
      } else {
        // Web mode fallback
        logs.set(['Running in web mode - no backend logs available']);
      }
    } catch (error) {
      console.error('Failed to fetch debug logs:', error);
      logs.set([`Error fetching logs: ${error}`]);
    } finally {
      isRefreshing.set(false);
    }
  }

  function clear(): void {
    logs.set([]);
  }

  return {
    logs: { subscribe: logs.subscribe },
    isRefreshing: { subscribe: isRefreshing.subscribe },
    fetch,
    clear,
  };
}

// Singleton instance for shared state
export const debugLogsService = createDebugLogsService();
