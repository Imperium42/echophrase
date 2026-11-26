// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach } from 'vitest';

const held = new Set<string>();
const plugin = {
  register: vi.fn(async (accelerator: string) => {
    if (held.has(accelerator)) throw new Error('HotKey already registered');
    held.add(accelerator);
  }),
  unregister: vi.fn(async (accelerator: string) => {
    held.delete(accelerator);
  }),
  unregisterAll: vi.fn(async () => held.clear()),
};
vi.mock('@tauri-apps/plugin-global-shortcut', () => plugin);
vi.mock('$lib/ipc/invoke', () => ({ mustInvoke: vi.fn() }));

const service = await import('./keyboardService');

describe('isShortcutAvailable (the hotkey recorder checks before accepting a combo)', () => {
  beforeEach(async () => {
    await service.cleanup();
    held.clear();
    vi.clearAllMocks();
    await service.init();
  });

  it('is true for a free combo and does not keep it registered', async () => {
    expect(await service.isShortcutAvailable('ctrl+shift+k')).toBe(true);
    expect(held.has('CommandOrControl+Shift+K')).toBe(false);
  });

  it('is false when another app holds the combo', async () => {
    held.add('CommandOrControl+Shift+Space');
    expect(await service.isShortcutAvailable('ctrl+shift+space')).toBe(false);
  });

  it('is true for our own active hotkey (re-picking the same combo)', async () => {
    await service.registerShortcut('shift+z', () => {});
    expect(await service.isShortcutAvailable('shift+z')).toBe(true);
    expect(held.has('Shift+Z')).toBe(true); // still ours, not torn down by the probe
  });
});
