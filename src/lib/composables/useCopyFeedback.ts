// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Reusable copy-to-clipboard with visual feedback composable
 * Handles the common pattern: copy text, show "Copied!" feedback, auto-reset
 */

import { writable, type Readable, type Writable } from 'svelte/store';
import { useClipboard } from './useClipboard';

export interface CopyFeedbackOptions {
  /** How long to show "Copied!" feedback in ms (default: 2000) */
  feedbackDuration?: number;
}

export interface CopyFeedbackState {
  /** Whether the copy feedback is currently showing */
  isCopied: Readable<boolean>;
}

export interface CopyFeedbackActions {
  /** Copy text to clipboard and trigger feedback */
  copy: (text: string) => Promise<void>;
  /** Cleanup timeouts (call in onDestroy) */
  cleanup: () => void;
}

export type CopyFeedback = CopyFeedbackState & CopyFeedbackActions;

// ---------------------------------------------------------------------------
// Internal: a writable that auto-resets to a default after `durationMs`. Used
// by both `useCopyFeedback` (boolean) and `useKeyedCopyFeedback` (string) to
// avoid duplicating the timeout-clear/reschedule dance.
// ---------------------------------------------------------------------------
interface TimedValue<T> {
  store: Writable<T>;
  /** Set the value, then reset to `defaultValue` after `durationMs`. */
  setFor: (value: T) => void;
  /** Cancel any pending reset and clear the timer (no value change). */
  cleanup: () => void;
}

function useTimedValue<T>(defaultValue: T, durationMs: number): TimedValue<T> {
  const store = writable<T>(defaultValue);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clear() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  function setFor(value: T) {
    store.set(value);
    clear();
    timer = setTimeout(() => {
      store.set(defaultValue);
      timer = null;
    }, durationMs);
  }

  return { store, setFor, cleanup: clear };
}

/**
 * Creates a copy feedback controller for buttons that copy text.
 *
 * @example
 * ```svelte
 * <script>
 *   const copyFeedback = useCopyFeedback();
 * </script>
 *
 * <button on:click={() => copyFeedback.copy(text)}>
 *   {#if $copyFeedback.isCopied}
 *     Copied!
 *   {:else}
 *     Copy
 *   {/if}
 * </button>
 * ```
 */
export function useCopyFeedback(options: CopyFeedbackOptions = {}): CopyFeedback {
  const { feedbackDuration = 2000 } = options;
  const flag = useTimedValue<boolean>(false, feedbackDuration);
  const clipboard = useClipboard();

  async function copy(text: string): Promise<void> {
    try {
      await clipboard.copyWithFeedback(
        text,
        () => flag.setFor(true),
        err => console.error('Failed to copy:', err)
      );
    } catch (error) {
      console.error('Failed to copy:', error);
    }
  }

  return {
    isCopied: { subscribe: flag.store.subscribe },
    copy,
    cleanup: flag.cleanup,
  };
}

/**
 * Keyed variant for components with multiple copy buttons.
 * Tracks which key was last copied so only one shows "Copied!" at a time.
 * On failure, the key is set to `${key}:error` briefly so the caller can
 * show distinct error feedback (e.g. "Failed!") instead of a silent no-op.
 */
export interface KeyedCopyFeedback {
  /** Store containing the currently copied key (empty string = none) */
  copiedKey: Readable<string>;
  /** Copy text and set the active key */
  copy: (key: string, text: string) => Promise<void>;
  /** Cleanup timeouts (call in onDestroy) */
  cleanup: () => void;
}

export function useKeyedCopyFeedback(options: CopyFeedbackOptions = {}): KeyedCopyFeedback {
  const { feedbackDuration = 2000 } = options;
  const flag = useTimedValue<string>('', feedbackDuration);
  const clipboard = useClipboard();

  async function copy(key: string, text: string): Promise<void> {
    try {
      await clipboard.copyWithFeedback(
        text,
        () => flag.setFor(key),
        err => {
          console.error(`[Clipboard] Failed to copy ${key}:`, err);
          flag.setFor(`${key}:error`);
        }
      );
    } catch (error) {
      console.error(`[Clipboard] Failed to copy ${key}:`, error);
      flag.setFor(`${key}:error`);
    }
  }

  return {
    copiedKey: { subscribe: flag.store.subscribe },
    copy,
    cleanup: flag.cleanup,
  };
}
