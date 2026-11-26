// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
  pageStore,
  showSettingsPanel,
  settingsInitialSection,
  settingsInitialSearch,
} from './pageStore';

describe('pageStore settings search deep-link', () => {
  beforeEach(() => {
    pageStore.closeSettings();
  });

  it('opens settings with a pre-filled search query', () => {
    pageStore.openSettingsWithSearch('input');
    expect(get(showSettingsPanel)).toBe(true);
    expect(get(settingsInitialSearch)).toBe('input');
    expect(get(settingsInitialSection)).toBeNull();
  });

  it('clears the search query on close', () => {
    pageStore.openSettingsWithSearch('input');
    pageStore.closeSettings();
    expect(get(showSettingsPanel)).toBe(false);
    expect(get(settingsInitialSearch)).toBeNull();
  });

  it('clears the search query when toggled closed', () => {
    pageStore.openSettingsWithSearch('input');
    pageStore.toggleSettings();
    expect(get(settingsInitialSearch)).toBeNull();
  });

  it('a plain section open does not carry a stale search', () => {
    pageStore.openSettingsWithSearch('input');
    pageStore.closeSettings();
    pageStore.openSettings('gpu-acceleration');
    expect(get(settingsInitialSearch)).toBeNull();
    expect(get(settingsInitialSection)).toBe('gpu-acceleration');
  });
});
