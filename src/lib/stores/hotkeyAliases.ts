// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Hotkey alias store - cosmetic per-key display names.
 *
 * A user can rename an individual key so the UI labels it the way their
 * keyboard does. Eg a mouse-side button that reports as `f15` can be aliased to
 * "M5", and every hotkey label in the app then reads "M5" instead of "F15".
 *
 * This is DISPLAY ONLY. The stored hotkey string and the registered OS
 * accelerator are untouched - aliasing `f15` does not rebind anything.
 *
 * Two access paths, mirroring platformCache:
 *  - `hotkeyAliases` (svelte store) for components, so labels re-render when an
 *    alias changes.
 *  - `getHotkeyAlias()` (sync) for `hotkeyFormatter`, which is a pure-ish util
 *    called from non-reactive code too.
 *
 * Keys are normalized to lowercase to match the canonical token form used in
 * the hotkey string (`ctrl+shift+space`). Rust normalizes the same way on save
 * (see `normalize_hotkey_aliases` in settings/mod.rs).
 */

import { writable, get } from 'svelte/store';
import { getSettings, updateSetting } from '$lib/ipc/settings';

/** Longest accepted alias. Mirrors HOTKEY_ALIAS_MAX_LEN in settings/mod.rs. */
export const HOTKEY_ALIAS_MAX_LEN = 12;

export type HotkeyAliasMap = Record<string, string>;

const store = writable<HotkeyAliasMap>({});

/** Sync snapshot for the formatter (which is not a component). */
let snapshot: HotkeyAliasMap = {};
store.subscribe(value => {
  snapshot = value;
});

/**
 * Normalize an alias map: lowercase keys, trimmed and length-capped values,
 * blank aliases dropped (clearing the field is how a user removes an alias).
 */
function normalize(aliases: HotkeyAliasMap): HotkeyAliasMap {
  const result: HotkeyAliasMap = {};
  for (const [key, alias] of Object.entries(aliases)) {
    const trimmed = alias.trim().slice(0, HOTKEY_ALIAS_MAX_LEN);
    if (!trimmed) continue;
    result[key.trim().toLowerCase()] = trimmed;
  }
  return result;
}

/**
 * Look up the alias for a single key token, or undefined if none is set.
 * Sync - safe to call from the formatter.
 */
export function getHotkeyAlias(token: string): string | undefined {
  return snapshot[token.trim().toLowerCase()];
}

export const hotkeyAliases = {
  subscribe: store.subscribe,

  /** Load persisted aliases from Tauri settings. Called once at app init. */
  async init(): Promise<void> {
    const settings = await getSettings();
    store.set(normalize(settings.hotkey_aliases ?? {}));
  },

  /**
   * Set (or clear, with an empty/blank alias) the display name for one key.
   * Persists the whole map, since the setting is a single map-valued field.
   */
  async setAlias(token: string, alias: string): Promise<void> {
    const key = token.trim().toLowerCase();
    if (!key) return;

    const next = { ...get(store) };
    const trimmed = alias.trim().slice(0, HOTKEY_ALIAS_MAX_LEN);
    if (trimmed) {
      next[key] = trimmed;
    } else {
      delete next[key];
    }

    store.set(next);
    await updateSetting('hotkey_aliases', next);
  },
};
