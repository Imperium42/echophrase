// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable } from 'svelte/store';

// Simple browser detection
const browser = typeof window !== 'undefined';

export type Theme = 'light' | 'dark' | 'system';

// Create theme store
function createThemeStore() {
  const { subscribe, set, update } = writable<Theme>('system');

  return {
    subscribe,
    set,
    update,

    // Initialize theme from localStorage or system preference
    init: () => {
      if (browser) {
        const saved = localStorage.getItem('echophrase-theme') as Theme;
        if (saved && ['light', 'dark', 'system'].includes(saved)) {
          set(saved);
          applyTheme(saved);
        } else {
          // Default to system preference
          applyTheme('system');
        }
      }
    },

    // Set theme and persist to localStorage
    setTheme: (theme: Theme) => {
      if (browser) {
        localStorage.setItem('echophrase-theme', theme);
        applyTheme(theme);
      }
      set(theme);
    },
  };
}

// Apply theme to document
function applyTheme(theme: Theme) {
  if (!browser) return;

  const root = document.documentElement;

  if (theme === 'system') {
    const systemTheme = window.matchMedia('(prefers-color-scheme: dark)').matches
      ? 'dark'
      : 'light';
    root.classList.toggle('dark', systemTheme === 'dark');
  } else {
    root.classList.toggle('dark', theme === 'dark');
  }
}

export const theme = createThemeStore();

// Listen for system theme changes
if (browser) {
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    theme.subscribe(currentTheme => {
      if (currentTheme === 'system') {
        applyTheme('system');
      }
    })();
  });
}
