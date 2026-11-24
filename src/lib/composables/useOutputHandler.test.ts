// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const clip = vi.hoisted(() => ({
  copyToClipboard: vi.fn(),
  readClipboard: vi.fn(),
}));
const tauri = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock('$lib/composables/useClipboard', () => ({ useClipboard: () => clip }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: tauri.invoke }));
vi.mock('$lib/utils/platform', () => ({ isWSLAsync: async () => false }));
vi.mock('$lib/composables/useAccessibilityPermission', () => ({
  isAccessibilityError: () => false,
  refreshAccessibilityPermission: vi.fn(),
  showAccessibilityModal: { set: vi.fn() },
}));
vi.mock('$lib/composables/useTauriSettings', async () => {
  const { writable } = await import('svelte/store');
  return { tauriSettings: writable({}) };
});

import { useOutputHandler } from './useOutputHandler';

describe('paste at cursor with restore', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    window.__TAURI_INTERNALS__ = {};
    clip.copyToClipboard.mockReset().mockResolvedValue(undefined);
    clip.readClipboard.mockReset();
    tauri.invoke.mockReset().mockResolvedValue(undefined);
  });
  afterEach(() => {
    vi.useRealTimers();
    delete window.__TAURI_INTERNALS__;
  });

  async function paste(keepOnClipboard: boolean) {
    const p = useOutputHandler().handleOutput(
      'hello',
      'paste_at_cursor',
      undefined,
      keepOnClipboard
    );
    await vi.runAllTimersAsync();
    return p;
  }

  it('restores the previous clipboard text after pasting', async () => {
    clip.readClipboard.mockResolvedValue('previous');
    const result = await paste(false);
    expect(result.success).toBe(true);
    expect(clip.copyToClipboard.mock.calls.map(c => c[0])).toEqual(['hello', 'previous']);
  });

  it('does not wipe the clipboard when the previous content is not text', async () => {
    clip.readClipboard.mockResolvedValue(null);
    const result = await paste(false);
    expect(result.success).toBe(true);
    expect(clip.copyToClipboard.mock.calls.map(c => c[0])).toEqual(['hello']);
  });

  it('still reports success when the restore write throws', async () => {
    clip.readClipboard.mockResolvedValue('previous');
    clip.copyToClipboard.mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error('boom'));
    const result = await paste(false);
    expect(result.success).toBe(true);
  });

  it('leaves the transcript and never reads the clipboard when keepOnClipboard', async () => {
    await paste(true);
    expect(clip.readClipboard).not.toHaveBeenCalled();
    expect(clip.copyToClipboard.mock.calls.map(c => c[0])).toEqual(['hello']);
  });
});
