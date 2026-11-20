// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * promiseWithTimeout
 *
 * Races a promise against a timeout so a stalled/hung async operation can
 * never wedge the caller forever. Used by useRecording's open mic chunk
 * processing to make sure a slow or stuck CPU decode can't permanently stop
 * the open-mic loop.
 */

/** Error thrown when `promiseWithTimeout` hits its deadline before `promise` settles. */
export class TimeoutError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'TimeoutError';
  }
}

/**
 * Resolve/reject with `promise`, or reject with a `TimeoutError` if it hasn't
 * settled within `timeoutMs`. The original promise is not cancelled (JS has
 * no cancellation primitive for arbitrary promises) - it's just no longer
 * awaited, so a late resolution is silently ignored by the caller.
 */
export function promiseWithTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  message = `Operation timed out after ${timeoutMs}ms`
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new TimeoutError(message));
    }, timeoutMs);

    promise
      .then(value => {
        clearTimeout(timer);
        resolve(value);
      })
      .catch(error => {
        clearTimeout(timer);
        reject(error);
      });
  });
}
