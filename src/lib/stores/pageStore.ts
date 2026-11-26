// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Page-level state store
 * Centralized modal/panel visibility management for the main page
 */

import { writable, get } from 'svelte/store';

// Modal visibility states - exported directly for $ prefix usage
export const showSettingsPanel = writable(false);
export const showImportCustomModelModal = writable(false);

// Settings panel initial section (for deep-linking to specific section)
export const settingsInitialSection = writable<string | null>(null);

// Settings panel initial search query (opens with the search box pre-filled)
export const settingsInitialSearch = writable<string | null>(null);

// Where closing the import custom model modal should return to (eg reopen the
// model picker it was launched from). Null = just close (the launcher is
// still open behind it, or there is nowhere to go back to).
let importModalReturnTo: (() => void) | null = null;

// Action functions
export const pageStore = {
  // Getters (for keyboard service)
  getShowSettingsPanel: () => get(showSettingsPanel),

  // Settings panel actions
  openSettings: (section?: string) => {
    if (section) {
      settingsInitialSection.set(section);
    }
    showSettingsPanel.set(true);
  },
  // Open settings filtered by a search query (eg "input"), like the user typed it
  openSettingsWithSearch: (query: string) => {
    settingsInitialSection.set(null);
    settingsInitialSearch.set(query);
    showSettingsPanel.set(true);
  },
  closeSettings: () => {
    showSettingsPanel.set(false);
    settingsInitialSection.set(null);
    settingsInitialSearch.set(null);
  },
  toggleSettings: () => {
    showSettingsPanel.update(v => !v);
    if (!get(showSettingsPanel)) {
      settingsInitialSection.set(null);
      settingsInitialSearch.set(null);
    }
  },

  // Import custom model modal actions. `returnTo` makes close/ESC/X go
  // back one layer (eg reopen the model picker) instead of dropping to the
  // main screen.
  openImportCustomModelModal: (opts?: { returnTo?: () => void }) => {
    importModalReturnTo = opts?.returnTo ?? null;
    showImportCustomModelModal.set(true);
  },
  // skipReturn: after a successful import the user already has what they came
  // for - close everything instead of reopening the launcher.
  closeImportCustomModelModal: (opts?: { skipReturn?: boolean }) => {
    showImportCustomModelModal.set(false);
    const returnTo = importModalReturnTo;
    importModalReturnTo = null;
    if (!opts?.skipReturn) returnTo?.();
  },

  // Check if any modal is open
  isAnyModalOpen: () => get(showSettingsPanel) || get(showImportCustomModelModal),
};
