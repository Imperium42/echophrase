// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Hotkey persistence utilities backed by Tauri settings (YAML).
 * Single source of truth - no more localStorage split-brain.
 */

import { getSettings, updateSetting } from '$lib/ipc/settings';
import { platformCache } from '$lib/stores/platformCache';

const DEFAULT_HOTKEY = 'ctrl+shift+space';
// macOS defaults to the dedicated Dictation (mic) key. We remap it to F13 at the
// HID layer and display it as "F5" (see macos_dictation.rs / useHotkeys.ts).
const DEFAULT_HOTKEY_MAC = 'f13';
const LOCAL_STORAGE_KEY = 'echophrase-hotkey';

/**
 * Platform-aware default hotkey. macOS defaults to Cmd+Shift+A (⇧⌘A); other
 * platforms to Ctrl+Shift+Space. Mirrors the Rust backend default in settings/mod.rs.
 */
export function getDefaultHotkey(): string {
  return platformCache.os.toLowerCase().includes('mac') ? DEFAULT_HOTKEY_MAC : DEFAULT_HOTKEY;
}

/**
 * Load hotkey from Tauri settings (async).
 * On first call, migrates any existing localStorage value.
 */
export async function getHotkey(): Promise<string> {
  await migrateFromLocalStorage();
  const settings = await getSettings();
  return settings.hotkey || getDefaultHotkey();
}

/**
 * Save hotkey to Tauri settings.
 */
export async function setHotkey(hotkey: string): Promise<void> {
  await updateSetting('hotkey', hotkey.toLowerCase());
}

/**
 * Reset hotkey to default in Tauri settings.
 */
export async function resetHotkey(): Promise<void> {
  await setHotkey(getDefaultHotkey());
}

/**
 * One-time migration: if localStorage has a custom hotkey and Tauri settings
 * still has the default, adopt the localStorage value then delete it.
 */
async function migrateFromLocalStorage(): Promise<void> {
  if (typeof window === 'undefined') return;

  const raw = localStorage.getItem(LOCAL_STORAGE_KEY);
  if (!raw) return;

  try {
    const parsed = JSON.parse(raw);
    const lsHotkey: string | undefined = parsed?.currentHotkey;

    if (lsHotkey && lsHotkey !== DEFAULT_HOTKEY) {
      const settings = await getSettings();
      // Only migrate if Tauri settings still has the default
      if (!settings.hotkey || settings.hotkey.toLowerCase() === DEFAULT_HOTKEY) {
        await updateSetting('hotkey', lsHotkey.toLowerCase());
        console.log('[hotkeyStorage] Migrated hotkey from localStorage:', lsHotkey);
      }
    }
  } catch {
    // Malformed localStorage value - just clean it up
  }

  localStorage.removeItem(LOCAL_STORAGE_KEY);
}

/** Default hotkey exported for external use */
export { DEFAULT_HOTKEY };
