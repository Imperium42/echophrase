// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { createViteLicensePlugin } from "rollup-license-plugin";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;
// Single source of truth for the dev port; scripts/env/load.mjs validates it and passes
// the same value to Tauri. HMR takes the next port up. Default matches tauri.conf.json5.
// @ts-expect-error process is a nodejs global
const devPort = Number(process.env.DEV_PORT) || 8420;
const hmrPort = devPort + 1;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    tailwindcss(),
    sveltekit(),
    // Emit a manifest of the third-party npm packages Vite actually bundles into
    // the shipped webview (name, version, SPDX license, license text), consumed by
    // `scripts/gen-attribution.mjs` to build the /attribution page data.
    //
    // GATED behind GEN_ATTRIBUTION=1 (set only by gen-attribution.mjs) so it never
    // runs during normal dev or the release `tauri build`: the plugin HARD-FAILS on
    // any dep it can't resolve a license for, and we must not let that break a
    // release. Attribution regeneration is an explicit, separate step.
    // @ts-expect-error process is a nodejs global
    ...(process.env.GEN_ATTRIBUTION === "1"
      ? [
          createViteLicensePlugin({
            outputFilename: "oss-licenses.json",
            // Fall back to canonical SPDX license text when a package ships none.
            replenishDefaultLicenseTexts: true,
            // Packages that ship a LICENSE file but omit the `license` field in
            // package.json (the plugin can't infer SPDX from text alone).
            // Verified by reading each package's bundled LICENSE file.
            licenseOverrides: {
              // MIT - node_modules/svelte-toolbelt/LICENSE (Hunter Johnston, Thomas G. Lopes)
              "svelte-toolbelt@0.10.6": "MIT",
            },
          }),
        ]
      : []),
  ],

  // Suppress source maps for Vite's pre-bundled node_modules deps.
  //
  // Without this, each bundled dep (node_modules/.vite/deps/*.js) emits a
  // `//# sourceMappingURL=X.js.map` comment, and the Tauri webview tries to
  // eagerly fetch the sidecar .js.map. Those fetches 404 with "Connection
  // refused" under WSL dev (72+ red errors in DevTools console on app start).
  //
  // This only disables source maps for PRE-BUNDLED third-party deps. Our own
  // app code (Svelte components, TypeScript, stores) still gets full source
  // maps for debugging.
  optimizeDeps: {
    // Vite 8 pre-bundles deps with Rolldown (esbuildOptions is deprecated).
    rolldownOptions: {
      output: {
        sourcemap: false,
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: devPort,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: hmrPort,
        }
      : {
          protocol: "ws",
          port: hmrPort,
          timeout: 30000,
          overlay: true,
          // Keep trying to reconnect
          clientPort: hmrPort,
        },
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
