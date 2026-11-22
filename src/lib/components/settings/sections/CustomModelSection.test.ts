// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/svelte';
import { writable } from 'svelte/store';

/**
 * Custom Models is free for everyone: the section must render the import
 * button directly, with no tier badge or upgrade teaser in front of it.
 *
 * Uses the real settings context (via `CustomModelSectionTestHost.svelte`,
 * a test-only harness that calls the real `setSettingsContext`) rather than
 * mocking `../context`, and mocks only the Tauri-backed service/IPC layer
 * so the component can mount in jsdom without a live backend.
 *
 * NOTE: the component is imported dynamically inside each test, after the
 * vi.mock() calls below run, rather than via a static top-level import.
 * Under `bunx --bun vitest`, vi.mock hoisting of static imports did not
 * reliably apply (we now run vitest on Node, see vitest.config.js) - the dynamic
 * `await import(...)` pattern sidesteps that by registering the mocks
 * before the module graph is first loaded.
 */

vi.mock('$lib/stores/pageStore', () => ({
  pageStore: {
    openImportCustomModelModal: vi.fn(),
  },
  showImportCustomModelModal: writable(false),
}));

vi.mock('$lib/services/customModelService', () => ({
  customModelChanged: writable(0),
  listCustomModels: vi.fn(async () => []),
  removeCustomModel: vi.fn(async () => {}),
}));

vi.mock('$lib/stores/modelStore', () => ({
  modelStore: writable({ selectedModel: null }),
}));

vi.mock('$lib/ipc/invoke', () => ({
  mustInvoke: vi.fn(async () => ({ custom_model_path: null })),
}));

vi.mock('$lib/utils/launchUrl', () => ({
  launchUrl: vi.fn(),
}));

// The real icon registry imports the full @lucide/svelte barrel (~1,600
// components), which took ~50s to compile cold and blew the test timeout.
vi.mock('$lib/icons', async () => ({
  Icon: (await import('./CustomModelSectionIconStub.svelte')).default,
}));

vi.mock('$lib/utils/docsUrl', () => ({
  docsUrl: (path: string) => `https://docs.echophrase.com${path}`,
}));

function makeContext() {
  return {
    settings: writable(null),
    updateSetting: vi.fn(async () => {}),
    availableModels: writable([]),
    isLoading: writable(false),
  };
}

describe('CustomModelSection', () => {
  it('renders the import button with no tier badge', async () => {
    const { default: CustomModelSectionTestHost } =
      await import('./CustomModelSectionTestHost.svelte');
    const { container } = render(CustomModelSectionTestHost, {
      props: { context: makeContext() },
    });
    expect(container.querySelector('.import-button')).toBeInTheDocument();
    expect(container.querySelector('.feature-badge')).not.toBeInTheDocument();
  }, 15000);
});
