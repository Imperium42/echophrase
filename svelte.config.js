// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  // Routes <style> blocks through PostCSS during preprocess. Historically
  // also a workaround for a vite-plugin-svelte v6 cold-cache race (raw
  // .svelte source fed to PostCSS as CSS) - fixed upstream in v7.1.x, but
  // vitePreprocess remains the standard setup for lang="postcss" styles.
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'static',
      assets: 'static',
      fallback: 'index.html',
    }),
  },
};

export default config;
