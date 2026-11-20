// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

const getManagedInstall = vi.fn();
const check = vi.fn();

vi.mock('$lib/utils/managedInstall', () => ({
  getManagedInstall: () => getManagedInstall(),
  isManagedInstall: async () => (await getManagedInstall()).managed,
}));
vi.mock('@tauri-apps/plugin-updater', () => ({ check: () => check() }));

import { checkForUpdates, managedUpgrade } from './updater';

const BREW = {
  managed: true,
  manager: 'homebrew',
  upgradeCommand: 'brew upgrade --cask echophrase',
};

describe('checkForUpdates on a package-manager install', () => {
  beforeEach(() => {
    getManagedInstall.mockReset();
    check.mockReset();
    managedUpgrade.set(null);
  });

  it('reports a newer build and the upgrade command instead of "up to date"', async () => {
    getManagedInstall.mockResolvedValue(BREW);
    check.mockResolvedValue({ available: true, version: '0.9.7' });

    expect(await checkForUpdates(false)).toBe(true);
    expect(get(managedUpgrade)).toEqual({
      version: '0.9.7',
      command: 'brew upgrade --cask echophrase',
    });
  });

  it('only says up to date when the lookup confirms it', async () => {
    getManagedInstall.mockResolvedValue(BREW);
    check.mockResolvedValue(null);

    expect(await checkForUpdates(false)).toBe(false);
    expect(get(managedUpgrade)).toBeNull();
  });

  it('reports a failed lookup as a failure, never as "up to date"', async () => {
    getManagedInstall.mockResolvedValue(BREW);
    check.mockRejectedValue(new Error('offline'));

    expect(await checkForUpdates(false)).toBeNull();
  });

  it('stays quiet on the silent startup check', async () => {
    getManagedInstall.mockResolvedValue(BREW);

    expect(await checkForUpdates(true)).toBe(false);
    expect(check).not.toHaveBeenCalled();
  });
});
