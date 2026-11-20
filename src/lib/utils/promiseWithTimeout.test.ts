// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { promiseWithTimeout, TimeoutError } from './promiseWithTimeout';

describe('promiseWithTimeout', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('resolves with the underlying value when it settles before the deadline', async () => {
    const promise = promiseWithTimeout(Promise.resolve('ok'), 1000);
    await vi.advanceTimersByTimeAsync(0);
    await expect(promise).resolves.toBe('ok');
  });

  it('rejects with the underlying error when it rejects before the deadline', async () => {
    // Reject on a deferred tick (rather than an already-rejected promise) so
    // promiseWithTimeout's .catch is attached before the rejection actually
    // fires, avoiding a spurious "unhandled rejection" warning in the runner.
    const inner = new Promise<never>((_resolve, reject) => {
      setTimeout(() => reject(new Error('boom')), 10);
    });
    const promise = promiseWithTimeout(inner, 1000);
    const assertion = expect(promise).rejects.toThrow('boom');
    await vi.advanceTimersByTimeAsync(10);
    await assertion;
  });

  it('rejects with a TimeoutError once the deadline elapses without settling', async () => {
    const never = new Promise<string>(() => {});
    const promise = promiseWithTimeout(never, 1000, 'took too long');

    const assertion = expect(promise).rejects.toBeInstanceOf(TimeoutError);
    await vi.advanceTimersByTimeAsync(1000);
    await assertion;
  });

  it('uses the provided message on timeout', async () => {
    const never = new Promise<string>(() => {});
    const promise = promiseWithTimeout(never, 500, 'chunk decode took too long');

    const assertion = expect(promise).rejects.toThrow('chunk decode took too long');
    await vi.advanceTimersByTimeAsync(500);
    await assertion;
  });

  it('does not fire the timeout after the promise already resolved', async () => {
    const promise = promiseWithTimeout(Promise.resolve(42), 1000);
    await vi.advanceTimersByTimeAsync(0);
    await expect(promise).resolves.toBe(42);
    // Advancing well past the deadline must not throw/reject anything further.
    await vi.advanceTimersByTimeAsync(5000);
  });
});
