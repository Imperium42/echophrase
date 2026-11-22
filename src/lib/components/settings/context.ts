// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Settings Context Provider
 *
 * Provides settings management to all settings components
 * via Svelte context, avoiding prop drilling.
 */

import { getContext, setContext } from 'svelte';
import type { Readable } from 'svelte/store';
import type { TauriSettings } from '$lib/composables/useTauriSettings';

/**
 * Context interface for settings components
 */
export interface SettingsContext {
  /** Current settings values */
  settings: Readable<TauriSettings | null>;
  /** Update a single setting (key-typed value) */
  updateSetting: <K extends keyof TauriSettings>(key: K, value: TauriSettings[K]) => Promise<void>;
  /** Loading state */
  isLoading: Readable<boolean>;
}

const SETTINGS_CONTEXT_KEY = Symbol('settings-context');

/**
 * Set the settings context at the root of the settings modal
 */
export function setSettingsContext(ctx: SettingsContext): void {
  setContext(SETTINGS_CONTEXT_KEY, ctx);
}

/**
 * Get the settings context from any child component
 */
export function getSettingsContext(): SettingsContext {
  const ctx = getContext<SettingsContext>(SETTINGS_CONTEXT_KEY);
  if (!ctx) {
    throw new Error('getSettingsContext must be called within a SettingsModal');
  }
  return ctx;
}
