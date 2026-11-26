// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { mainKeyFromEvent } from './hotkeyKey';

const ev = (key: string, code: string) => ({ key, code });

describe('mainKeyFromEvent', () => {
  it('uses the physical code for F13-F24 even when the keymap renames the key', () => {
    // Stock Linux xkb maps F15 to XF86Launch6, so event.key is not "F15"
    expect(mainKeyFromEvent(ev('LaunchApplication7', 'F15'))).toBe('f15');
    expect(mainKeyFromEvent(ev('Unidentified', 'F13'))).toBe('f13');
    expect(mainKeyFromEvent(ev('F24', 'F24'))).toBe('f24');
  });

  it('keeps plain F1-F12 working', () => {
    expect(mainKeyFromEvent(ev('F5', 'F5'))).toBe('f5');
  });

  it('normalizes the spacebar', () => {
    expect(mainKeyFromEvent(ev(' ', 'Space'))).toBe('space');
    expect(mainKeyFromEvent(ev('Spacebar', 'Space'))).toBe('space');
  });

  it('falls back to the code for unidentified keys', () => {
    expect(mainKeyFromEvent(ev('Unidentified', 'KeyA'))).toBe('a');
    expect(mainKeyFromEvent(ev('Unidentified', 'Digit7'))).toBe('7');
  });

  it('lowercases ordinary keys and modifiers', () => {
    expect(mainKeyFromEvent(ev('A', 'KeyA'))).toBe('a');
    expect(mainKeyFromEvent(ev('Shift', 'ShiftLeft'))).toBe('shift');
  });

  it('does not treat media keys on the F-row as F-keys', () => {
    expect(mainKeyFromEvent(ev('AudioVolumeUp', 'AudioVolumeUp'))).toBe('audiovolumeup');
  });
});
