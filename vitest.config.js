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
  // Vitest runs on the Node runtime, NOT bun's (`bunx vitest`, never `bunx --bun vitest`).
  // Bun stays the package manager / script runner; only the test workers use Node.
  // Under `--bun` on Windows the worker pool deadlocked at startup (20 idle workers
  // for 14h on 2026-11-21) or exited 0 having run nothing. Known and unfixed upstream:
  // - https://github.com/oven-sh/bun/issues/10180 (Windows: hangs with --bun, works without; closed not planned)
  // - https://github.com/oven-sh/bun/issues/20762 (tinypool crash under bun, vitest 3.2)
  // - https://github.com/oven-sh/bun/issues/18236 (vitest hangs indefinitely under bun)
  // - https://github.com/vitest-dev/vitest/issues/12037 (vitest 5.1 forks pool fails to start on bun)
  // - https://github.com/oven-sh/bun/issues/27002 (threads pool segfaults on bun)
  // - https://github.com/oven-sh/bun/issues/46396 (~3x slower than Node)
  // Re-test before switching back.
  test: {
    include: ['src/**/*.{test,spec}.{js,ts}', 'tests/**/*.{test,spec}.{js,ts}'],
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./vitest.setup.ts'],
  }
});