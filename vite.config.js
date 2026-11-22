// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";
import { createViteLicensePlugin } from "rollup-license-plugin";

// Frontend COPYLEFT GUARD: the webview-side twin of the `accepted` list in
// src-tauri/about.toml. Keep both lists in sync. Permissive / notice-only
// licenses only. Never add GPL/AGPL/LGPL/SSPL - a copyleft package must break
// the build, not ship silently.
const ACCEPTED_LICENSES = new Set([
  "0BSD",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "ISC",
  "MIT",
  "MIT-0",
  "MPL-2.0",
  "Unicode-3.0",
  "Zlib",
]);

/**
 * True when an SPDX expression can be satisfied using ACCEPTED_LICENSES.
 * `A OR B` needs one side, `A AND B` needs both, `A WITH X` is judged by `A`.
 * Malformed expressions return false so they fail closed.
 * @param {string} expression
 * @returns {boolean}
 */
function isAcceptableLicense(expression) {
  const tokens = expression.match(/\(|\)|[^\s()]+/g) ?? [];
  let pos = 0;
  const take = () => tokens[pos++];
  const peek = () => tokens[pos];

  const parseAtom = () => {
    const token = take();
    if (token === "(") {
      const value = parseOr();
      if (take() !== ")") throw new Error("unclosed parenthesis");
      return value;
    }
    if (token === undefined || [")", "AND", "OR", "WITH"].includes(token)) {
      throw new Error("unexpected token");
    }
    const allowed = ACCEPTED_LICENSES.has(token);
    if (peek() === "WITH") {
      take();
      if (take() === undefined) throw new Error("missing exception id");
    }
    return allowed;
  };
  const parseAnd = () => {
    let value = parseAtom();
    while (peek() === "AND") {
      take();
      const rhs = parseAtom();
      value = value && rhs;
    }
    return value;
  };
  function parseOr() {
    let value = parseAnd();
    while (peek() === "OR") {
      take();
      const rhs = parseAnd();
      value = value || rhs;
    }
    return value;
  }

  try {
    const result = parseOr();
    return pos === tokens.length && result;
  } catch {
    return false;
  }
}

// Packages that ship a LICENSE file but omit the `license` field in package.json
// (the plugin can't infer SPDX from text alone). Verified by reading each
// package's bundled LICENSE file. Shared by the guard and the attribution plugin.
const licenseOverrides = {
  // MIT - node_modules/svelte-toolbelt/LICENSE (Hunter Johnston, Thomas G. Lopes)
  "svelte-toolbelt@0.10.6": "MIT",
};

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
    // Copyleft guard: runs on every production build (apply: "build", so never in
    // dev). Fails the build naming the package if any npm package bundled into the
    // webview has a license outside ACCEPTED_LICENSES. Mirrors src-tauri/about.toml.
    // Only bundled modules are checked (dev-only deps such as sharp never reach
    // renderChunk). No output file is emitted; the guard is a check only.
    createViteLicensePlugin({
      outputFilename: false,
      licenseOverrides,
      unacceptableLicenseTest: (license) => !isAcceptableLicense(license),
    }),
    // Emit a manifest of the third-party npm packages Vite actually bundles into
    // the shipped webview (name, version, SPDX license, license text), consumed by
    // `scripts/gen-attribution.mjs` to build the /attribution page data.
    //
    // GATED behind GEN_ATTRIBUTION=1 (set only by gen-attribution.mjs) so it never
    // runs during normal dev or the release `tauri build`: it reads license texts
    // and replenishes missing ones from spdx.org, which must stay an explicit,
    // separate step. The copyleft guard above is always on.
    // @ts-expect-error process is a nodejs global
    ...(process.env.GEN_ATTRIBUTION === "1"
      ? [
          createViteLicensePlugin({
            outputFilename: "oss-licenses.json",
            // Fall back to canonical SPDX license text when a package ships none.
            replenishDefaultLicenseTexts: true,
            licenseOverrides,
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
