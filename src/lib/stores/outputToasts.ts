// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Output toast store and helpers.
 *
 * Manages transient toast notifications shown after each transcription output
 * action (copy, paste, save, transform).
 */

import { writable } from 'svelte/store';
import { translate } from '$lib/locales';

// Matches CSS animation duration in app.css.
const OUTPUT_TOAST_DURATION_MS = 3000;
const HOVER_DISMISS_DELAY_MS = 1500;

// -------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------

export interface OutputToast {
  id: string;
  mode: string;
  label: string;
  text?: string;
  filePath?: string;
  originalText?: string;
  hovered?: boolean;
  createdAt: number;
}

// -------------------------------------------------------------------------
// Stores
// -------------------------------------------------------------------------

const outputToastsStore = writable<OutputToast[]>([]);
const lastToastsStore = writable<OutputToast[]>([]);

const toastTimers = new Map<string, ReturnType<typeof setTimeout>>();

// -------------------------------------------------------------------------
// Public store
// -------------------------------------------------------------------------

/** Subscribe to the current list of active output toasts. */
export const outputToasts = {
  subscribe: outputToastsStore.subscribe,
  setHovered: (id: string, hovered: boolean) => {
    outputToastsStore.update(toasts => toasts.map(t => (t.id === id ? { ...t, hovered } : t)));
  },
  dismiss: (id: string) => {
    outputToastsStore.update(toasts => {
      const toast = toasts.find(t => t.id === id);
      if (toast) {
        lastToastsStore.update(last => [toast, ...last.filter(t => t.id !== id)].slice(0, 1));
      }
      return toasts.filter(t => t.id !== id);
    });
  },
};

/** Subscribe to the last dismissed toast (for re-showing on area hover). */
export const lastOutputToasts = {
  subscribe: lastToastsStore.subscribe,
  clear: () => lastToastsStore.set([]),
};

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

/** Re-schedule a toast dismiss after hover ends. */
export function scheduleToastDismiss(id: string): void {
  const existing = toastTimers.get(id);
  if (existing) clearTimeout(existing);

  const timerId = setTimeout(() => {
    outputToastsStore.update(toasts => {
      const current = toasts.find(t => t.id === id);
      if (current?.hovered) {
        scheduleToastDismiss(id);
        return toasts;
      }
      if (current) {
        lastToastsStore.update(last =>
          [current, ...last.filter(t => t.id !== current.id)].slice(0, 1)
        );
      }
      return toasts.filter(t => t.id !== id);
    });
    toastTimers.delete(id);
  }, HOVER_DISMISS_DELAY_MS);

  toastTimers.set(id, timerId);
}

/** Add an output toast notification. */
export function addOutputToast(
  mode: string,
  metadata?: { text?: string; filePath?: string; originalText?: string }
): void {
  const labels: Record<string, string> = {
    paste_at_cursor: translate('toast.pasted'),
    copy_to_clipboard: translate('toast.copied'),
    save_as_file: translate('toast.saved'),
    transformed: translate('toast.transformed'),
  };
  const toast: OutputToast = {
    id: `${mode}-${Date.now()}`,
    mode,
    label: labels[mode] || mode,
    text: metadata?.text,
    filePath: metadata?.filePath,
    originalText: metadata?.originalText,
    hovered: false,
    createdAt: Date.now(),
  };
  outputToastsStore.update(toasts => [...toasts, toast]);

  const timerId = setTimeout(() => {
    outputToastsStore.update(toasts => {
      const current = toasts.find(t => t.id === toast.id);
      if (current?.hovered) {
        scheduleToastDismiss(toast.id);
        return toasts;
      }
      if (current) {
        lastToastsStore.update(last =>
          [current, ...last.filter(t => t.id !== current.id)].slice(0, 1)
        );
      }
      return toasts.filter(t => t.id !== toast.id);
    });
    toastTimers.delete(toast.id);
  }, OUTPUT_TOAST_DURATION_MS);

  toastTimers.set(toast.id, timerId);
}
