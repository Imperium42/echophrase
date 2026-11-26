// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Hotkey management store for keyboard shortcuts.
 * Backed by Tauri settings (YAML) - single source of truth.
 */
import { writable } from 'svelte/store';
import { getHotkey, setHotkey, DEFAULT_HOTKEY } from '$lib/utils/hotkeyStorage';

interface HotkeyState {
  currentHotkey: string;
  isRecording: boolean;
  recordedKeys: string[];
  /** True once the saved hotkey has been loaded from settings (or explicitly set). */
  initialized: boolean;
}

/**
 * Creates hotkey store with recording and persistence via Tauri settings
 */
function createHotkeyStore() {
  const initialState: HotkeyState = {
    currentHotkey: DEFAULT_HOTKEY, // Sync placeholder until async init completes
    isRecording: false,
    recordedKeys: [],
    initialized: false,
  };

  const { subscribe, update } = writable(initialState);

  return {
    subscribe,

    /** Load hotkey from Tauri settings (async) */
    async init() {
      const savedHotkey = await getHotkey();
      update(state => ({
        ...state,
        currentHotkey: savedHotkey,
        initialized: true,
      }));
    },

    /** Start recording a new hotkey */
    startRecording() {
      update(state => ({
        ...state,
        isRecording: true,
        recordedKeys: [],
      }));
    },

    /** Stop recording and save if we have keys */
    async stopRecording() {
      let newHotkey: string | null = null;

      update(state => {
        if (state.recordedKeys.length > 0) {
          newHotkey = state.recordedKeys.join('+').toLowerCase();
          return {
            ...state,
            currentHotkey: newHotkey,
            isRecording: false,
            recordedKeys: [],
          };
        }

        return {
          ...state,
          isRecording: false,
          recordedKeys: [],
        };
      });

      // Persist to Tauri settings outside the synchronous update
      if (newHotkey) {
        await setHotkey(newHotkey);
      }
    },

    /** Cancel recording without saving */
    cancelRecording() {
      update(state => ({
        ...state,
        isRecording: false,
        recordedKeys: [],
      }));
    },

    /** Set the recorded key combination (replaces any previous) */
    addRecordedKey(key: string) {
      update(state => {
        if (!state.isRecording) return state;

        const normalizedKey = key.toLowerCase();
        return {
          ...state,
          recordedKeys: [normalizedKey], // Replace, don't accumulate
        };
      });
    },
  };
}

export const hotkeyStore = createHotkeyStore();

export { formatHotkey } from '$lib/utils/hotkeyFormatter';
