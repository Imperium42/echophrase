// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));

export default defineConfig({
  plugins: [svelte({ hot: false })],
  resolve: {
    alias: {
      $lib: resolve(__dirname, 'src/lib'),
    },
    // Svelte 5 ships separate client/server builds selected via package export
    // conditions. Without forcing 'browser' here, vite-plugin-svelte compiles
    // components for SSR (index-server.js), and @testing-library/svelte's
    // mount() throws `lifecycle_function_unavailable` for every component test.
    conditions: ['browser'],
  },
  test: {
    include: ['src/**/*.{test,spec}.{js,ts}', 'tests/**/*.{test,spec}.{js,ts}'],
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./vitest.setup.ts'],
  }
});