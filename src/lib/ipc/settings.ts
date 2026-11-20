// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Settings IPC helpers
 *
 * `updateSetting` is the ONE way to change an individual setting. It sends a
 * patch containing only that key, so a write can never revert a field another
 * writer changed concurrently. Rust applies the patch atomically (see
 * AppSettings::update).
 */

import { mustInvoke } from './invoke';
import type { Settings } from '$lib/types/settings';
import { defaultSettings } from '$lib/types/settings';

export { defaultSettings };

/** Load all settings from the Rust backend. */
export async function getSettings(): Promise<Settings> {
  return mustInvoke<Settings>('get_settings');
}

/**
 * Persist a FULL settings object, replacing every field.
 *
 * Only for wholesale replacement (eg reset to defaults). For changing
 * individual settings use `updateSetting`/`updateSettings` - a full save
 * carries a snapshot of every other field and will revert concurrent changes.
 */
export async function saveSettings(s: Settings): Promise<void> {
  return mustInvoke<void>('save_settings', { settings: s });
}

/**
 * Apply a partial settings update atomically. Returns the resulting settings.
 *
 * Keys omitted from `patch` are left untouched by the backend.
 */
export async function updateSettings(patch: Partial<Settings>): Promise<Settings> {
  return mustInvoke<Settings>('update_settings_patch', { patch });
}

/** Atomically update a single settings key. Returns the updated settings. */
export async function updateSetting<K extends keyof Settings>(
  key: K,
  value: Settings[K]
): Promise<Settings> {
  return updateSettings({ [key]: value } as Partial<Settings>);
}
