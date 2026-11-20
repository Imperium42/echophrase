// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { formatHotkey, formatKeyToken, tokensToDisplay } from './hotkeyFormatter';
import * as platform from './platform';

/** Force isMac() to a known value for the current test. */
function setMac(value: boolean) {
  vi.spyOn(platform, 'isMac').mockReturnValue(value);
}

describe('hotkeyFormatter', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  describe('macOS (glyphs, no separators)', () => {
    beforeEach(() => setMac(true));

    it('renders ctrl as ⌘ because it binds to Command on macOS (Cmd last, per Apple order)', () => {
      expect(formatHotkey('ctrl+shift+space')).toBe('⇧ ⌘ Space');
    });

    it('renders alt as ⌥', () => {
      expect(formatHotkey('alt+f5')).toBe('⌥ F5');
    });

    it('renders explicit meta/cmd as ⌘', () => {
      expect(formatHotkey('meta+a')).toBe('⌘ A');
      expect(formatHotkey('cmd+a')).toBe('⌘ A');
    });

    it('dedupes ctrl + meta (both ⌘) into a single glyph', () => {
      expect(formatHotkey('ctrl+meta+a')).toBe('⌘ A');
    });

    it('sorts modifiers into canonical order regardless of input order', () => {
      // input shuffled; expected Apple order is ⌥ ⇧ ⌘ then key
      expect(formatHotkey('cmd+shift+alt+k')).toBe('⌥ ⇧ ⌘ K');
    });
  });

  describe('Windows/Linux (words, " + " separators)', () => {
    beforeEach(() => setMac(false));

    it('renders ctrl/shift/space as words', () => {
      expect(formatHotkey('ctrl+shift+space')).toBe('Ctrl + Shift + Space');
    });

    it('renders alt + function key', () => {
      expect(formatHotkey('alt+f5')).toBe('Alt + F5');
    });

    it('sorts into Ctrl Alt Shift order regardless of input order', () => {
      expect(formatHotkey('shift+alt+ctrl+k')).toBe('Ctrl + Alt + Shift + K');
    });
  });

  describe('formatKeyToken / tokensToDisplay', () => {
    it('formatKeyToken maps a single modifier per platform', () => {
      setMac(true);
      expect(formatKeyToken('ctrl')).toBe('⌘');
      setMac(false);
      expect(formatKeyToken('ctrl')).toBe('Ctrl');
    });

    it('tokensToDisplay handles a live-capture token array', () => {
      setMac(true);
      expect(tokensToDisplay(['ctrl', 'shift', 'space'])).toBe('⇧ ⌘ Space');
    });

    it('ignores empty tokens', () => {
      setMac(false);
      expect(tokensToDisplay(['ctrl', '', 'a'])).toBe('Ctrl + A');
    });
  });

  describe('key aliases', () => {
    it('renders an aliased key by its custom name', () => {
      setMac(false);
      expect(formatHotkey('f15', { aliases: { f15: 'M5' } })).toBe('M5');
    });

    it('keeps modifiers canonical and only aliases the key', () => {
      setMac(false);
      expect(formatHotkey('ctrl+shift+f15', { aliases: { f15: 'M5' } })).toBe('Ctrl + Shift + M5');
    });

    it('does not alias modifier tokens', () => {
      setMac(false);
      // An alias entry for a modifier is inert - Ctrl keeps its universal label.
      expect(formatHotkey('ctrl+a', { aliases: { ctrl: 'Nope' } })).toBe('Ctrl + A');
    });

    it('matches the alias key case-insensitively against the token', () => {
      setMac(false);
      expect(formatKeyToken('F15', { aliases: { f15: 'M5' } })).toBe('M5');
    });

    it('falls back to the built-in label when no alias is set for that key', () => {
      setMac(false);
      expect(formatHotkey('ctrl+f14', { aliases: { f15: 'M5' } })).toBe('Ctrl + F14');
    });

    it('an alias overrides the macOS f13 -> F5 dictation-key label', () => {
      setMac(true);
      expect(formatKeyToken('f13')).toBe('F5');
      expect(formatKeyToken('f13', { aliases: { f13: 'Mic' } })).toBe('Mic');
    });

    it('ignoreAlias renders the key real name, for the alias editor rows', () => {
      setMac(false);
      expect(formatKeyToken('f15', { aliases: { f15: 'M5' }, ignoreAlias: true })).toBe('F15');
    });
  });
});
