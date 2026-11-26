// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { get } from 'svelte/store';

vi.mock('hotkeys-js', () => ({ default: Object.assign(vi.fn(), { unbind: vi.fn() }) }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('$lib/utils/hotkeyStorage', () => ({
  getHotkey: vi.fn(async () => 'shift+z'),
  setHotkey: vi.fn(async () => {}),
  getDefaultHotkey: () => 'ctrl+shift+space',
  DEFAULT_HOTKEY: 'ctrl+shift+space',
}));
// "ctrl+shift+space" is held by another app; everything else is free.
vi.mock('$lib/services/globalShortcutService', () => ({
  isShortcutAvailable: vi.fn(async (combo: string) => combo !== 'ctrl+shift+space'),
  registerShortcut: vi.fn(async () => true),
  unregisterCurrent: vi.fn(async () => {}),
  hasActiveGlobalShortcut: () => false,
  cleanup: vi.fn(async () => {}),
  init: vi.fn(async () => true),
}));

import { useHotkeys } from './useHotkeys';
import * as globalShortcutService from '$lib/services/globalShortcutService';

function press(init: KeyboardEventInit) {
  window.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, ...init }));
}
const flush = () => vi.advanceTimersByTimeAsync(0);

describe('shared hotkey recorder (pill + Settings > Hotkeys + global hotkey)', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    useHotkeys().cancelHotkeyRecording();
    vi.useRealTimers();
  });

  it('a combo recorded in one instance shows, with its countdown, in every other', async () => {
    const pill = useHotkeys();
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    press({ key: 'k', code: 'KeyK', ctrlKey: true, shiftKey: true });
    await flush();
    expect(get(pill.recordedKeys)).toEqual(['ctrl+shift+k']);
    expect(get(pill.countdown)).toBe(3);
    await vi.advanceTimersByTimeAsync(1000);
    expect(get(pill.countdown)).toBe(2);
  });

  it('a combo another app holds shakes "in use" and is not recorded', async () => {
    const pill = useHotkeys();
    pill.startHotkeyRecording();
    press({ key: ' ', code: 'Space', ctrlKey: true, shiftKey: true });
    await flush();
    expect(get(pill.recordedKeys)).toEqual([]);
    expect(get(pill.hotkeyShake)).toBe(true);
    expect(get(useHotkeys().rejection)?.reason).toBe('inUse');
  });

  it('a bare function key (eg F15) is accepted', async () => {
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    press({ key: 'XF86Launch6', code: 'F15' });
    await flush();
    expect(get(settings.recordedKeys)).toEqual(['f15']);
  });

  it('Esc cancels the capture and never reaches the listeners that close Settings', async () => {
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    // Same phase/target as keyboardService's ESC handler (document, bubble)
    // and modalController's (window, bubble).
    const closeSettings = vi.fn();
    document.addEventListener('keydown', closeSettings);
    window.addEventListener('keydown', closeSettings);
    document.body.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
    );
    await flush();
    document.removeEventListener('keydown', closeSettings);
    window.removeEventListener('keydown', closeSettings);
    expect(get(settings.isRecordingHotkey)).toBe(false);
    expect(closeSettings).not.toHaveBeenCalled();
  });

  it('releases our own global hotkey while picking, so it cannot swallow keys', () => {
    useHotkeys().startHotkeyRecording();
    expect(globalShortcutService.unregisterCurrent).toHaveBeenCalled();
  });

  it('a click while it still says "Press keys..." cancels the pick', () => {
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    expect(get(settings.isRecordingHotkey)).toBe(false);
  });

  it('a click after a combo is recorded leaves the countdown running', async () => {
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    press({ key: 'k', code: 'KeyK', ctrlKey: true, shiftKey: true });
    await flush();
    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    expect(get(settings.isRecordingHotkey)).toBe(true);
    expect(get(settings.recordedKeys)).toEqual(['ctrl+shift+k']);
  });

  it('leaving the app (window blur) cancels the pick', () => {
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    window.dispatchEvent(new Event('blur'));
    expect(get(settings.isRecordingHotkey)).toBe(false);
  });

  it.each([
    ['c', 'reservedCopy'],
    ['v', 'reservedPaste'],
    ['a', 'reservedSelectAll'],
  ])('Ctrl+%s is refused as reserved (%s) and not recorded', async (key, reason) => {
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    press({ key, code: `Key${key.toUpperCase()}`, ctrlKey: true });
    await flush();
    expect(get(settings.recordedKeys)).toEqual([]);
    expect(get(settings.rejection)?.reason).toBe(reason);
    expect(get(settings.hotkeyShake)).toBe(true);
  });

  it('Ctrl+Shift+C is still allowed (only exact reserved combos are refused)', async () => {
    const settings = useHotkeys();
    settings.startHotkeyRecording();
    press({ key: 'C', code: 'KeyC', ctrlKey: true, shiftKey: true });
    await flush();
    expect(get(settings.recordedKeys)).toEqual(['ctrl+shift+c']);
  });

  it('Ctrl+Shift released with no third key flags the WSL hint; a real combo does not', async () => {
    const settings = useHotkeys();
    const up = (init: KeyboardEventInit) =>
      window.dispatchEvent(new KeyboardEvent('keyup', { bubbles: true, ...init }));

    settings.startHotkeyRecording();
    press({ key: 'Control', code: 'ControlLeft', ctrlKey: true });
    press({ key: 'Shift', code: 'ShiftLeft', ctrlKey: true, shiftKey: true });
    up({ key: 'Shift', code: 'ShiftLeft', ctrlKey: true });
    expect(get(settings.modifierChordWithoutKey)).toBe(false); // Ctrl still held
    up({ key: 'Control', code: 'ControlLeft' });
    expect(get(settings.modifierChordWithoutKey)).toBe(true);

    settings.cancelHotkeyRecording();
    settings.startHotkeyRecording();
    expect(get(settings.modifierChordWithoutKey)).toBe(false); // reset per pick
    press({ key: 'Control', code: 'ControlLeft', ctrlKey: true });
    press({ key: 'Shift', code: 'ShiftLeft', ctrlKey: true, shiftKey: true });
    press({ key: 'K', code: 'KeyK', ctrlKey: true, shiftKey: true });
    up({ key: 'Control', code: 'ControlLeft' });
    await flush();
    expect(get(settings.modifierChordWithoutKey)).toBe(false);
  });
});
