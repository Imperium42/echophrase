// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/** Physical F-key codes (F1-F24) as reported by `KeyboardEvent.code`. */
const FUNCTION_KEY_CODE = /^F([1-9]|1[0-9]|2[0-4])$/;

/**
 * Canonical lowercase token for the main key of a keydown event (hotkeys-js
 * format: "f15", "space", "a").
 *
 * F1-F24 come from `event.code` (the physical key), not `event.key`: stock
 * Linux keymaps rename F13+ (eg F15 -> XF86Launch6), so `event.key` would not
 * say "F15" even though the key is F15. Macro-key keyboards rely on these.
 */
export function mainKeyFromEvent(event: Pick<KeyboardEvent, 'key' | 'code'>): string {
  const code = event.code ?? '';
  if (FUNCTION_KEY_CODE.test(code)) return code.toLowerCase();

  const key = event.key.toLowerCase();
  if (key === ' ' || key === 'spacebar') return 'space';

  // Some keys come through unidentified (eg Tab in some webviews); the code
  // still names the physical key: KeyA -> a, Digit7 -> 7, Tab -> tab.
  if (key === 'unidentified' && code) {
    return code.replace(/^Key/, '').replace(/^Digit/, '').toLowerCase();
  }
  return key;
}
