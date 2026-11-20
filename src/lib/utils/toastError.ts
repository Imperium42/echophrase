// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Shared toast error helper.
 *
 * Wraps a try/catch around async action handlers that show user-visible error
 * toasts, so each call site doesn't have to re-implement the same pattern.
 *
 * Usage:
 *   import { withToastError } from '$lib/utils/toastError';
 *   await withToastError(() => myAsyncAction(), setErrorState);
 */

/**
 * Execute `fn`, calling `onError` with a string message if it throws.
 * Always logs the original error to the console.
 *
 * @param fn - Async action to run.
 * @param onError - Called with a human-readable message when `fn` throws.
 */
export async function withToastError(
  fn: () => Promise<unknown>,
  onError: (message: string) => void
): Promise<void> {
  try {
    await fn();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error('[withToastError]', err);
    onError(msg);
  }
}
