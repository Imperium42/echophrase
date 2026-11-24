// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Typed IPC invoke primitives
 *
 * Provides safe and strict wrappers around Tauri's invoke() so call sites
 * never import the raw API or cast to `any`.
 *
 * - `safeInvoke` - returns null on error (fire-and-forget style)
 * - `mustInvoke`  - throws on error (use when the caller must handle failure)
 */

/** Detect whether the Tauri webview is available */
export function isTauriAvailable(): boolean {
  return typeof window !== 'undefined' && typeof window.__TAURI_INTERNALS__ !== 'undefined';
}

/**
 * Invoke a Tauri command, returning `null` on any error.
 * Logs the error to the console so nothing is silently swallowed.
 *
 * Use this when a command failure is non-fatal and the UI can degrade
 * gracefully (e.g. optional analytics, background checks).
 */
export async function safeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>
): Promise<T | null> {
  if (!isTauriAvailable()) {
    console.warn(`[ipc] safeInvoke('${cmd}') skipped - not in Tauri context`);
    return null;
  }

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.error(`[ipc] safeInvoke('${cmd}') failed:`, err);
    return null;
  }
}

/**
 * Invoke a Tauri command, throwing on any error.
 *
 * Use this when command failure must propagate to the caller (e.g. saving
 * settings, downloading a model - the UI needs to show an error).
 */
export async function mustInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriAvailable()) {
    throw new Error(`[ipc] mustInvoke('${cmd}') called outside Tauri context`);
  }

  const { invoke } = await import('@tauri-apps/api/core');
  return await invoke<T>(cmd, args);
}
