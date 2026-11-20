// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Hotkey display formatting utilities
 *
 * Renders a stored hotkey string (hotkeys-js format, e.g. "ctrl+shift+space") for
 * display. Platform-aware: on macOS it uses native modifier glyphs (⌃ ⌥ ⇧ ⌘) with no
 * separators (e.g. "⌘⇧Space"); on Windows/Linux it uses words joined with " + "
 * (e.g. "Ctrl + Shift + Space").
 *
 * Individual keys can be renamed by the user (see `hotkeyAliases`), so a binding
 * on an unlabelled key like F15 can display as "M5". Aliases are cosmetic and
 * resolved here, which is why every label in the app must render through these
 * helpers rather than printing the raw hotkey string.
 *
 * IMPORTANT - why `ctrl` shows as ⌘ on macOS: `convertToAccelerator()`
 * (src/lib/services/keyboardService.ts) maps the `ctrl` token to Tauri's
 * `CommandOrControl`, which the OS resolves to Command on macOS. So the shortcut that
 * actually fires for a stored `ctrl+...` is Cmd+..., and the display must reflect that
 * or the UI contradicts the real binding.
 */
import { isMac } from './platform';
import { platformCache } from '$lib/stores/platformCache';
import { getHotkeyAlias } from '$lib/stores/hotkeyAliases';

/**
 * Are we on macOS? Prefer the backend-authoritative OS from platformCache
 * (populated from get_system_info at startup); fall back to the navigator-based
 * isMac() only before the cache initializes. navigator.platform is deprecated and
 * unreliable inside WKWebView on recent macOS, which made the Mac glyphs not render.
 */
function onMac(): boolean {
  if (platformCache.initialized) return platformCache.os.toLowerCase().includes('mac');
  return isMac();
}

/** Modifier tokens (normalized, lowercase) and how each renders per platform. */
interface ModifierDisplay {
  /** macOS glyph. */
  mac: string;
  /** Windows/Linux label. */
  other: string;
  /** Canonical sort weight on macOS (⌃ ⌥ ⇧ ⌘). */
  macOrder: number;
  /** Canonical sort weight on Windows/Linux (Ctrl Alt Shift Win). */
  otherOrder: number;
}

/**
 * Single source of truth for modifier rendering. Aliases collapse to the same display,
 * so `ctrl`/`control` (and on macOS, `meta`/`cmd`/`command`) dedupe to one glyph.
 * macOS canonical order is ⌃ ⌥ ⇧ ⌘; other platforms use Ctrl Alt Shift (Win/Super last).
 */
const MODIFIERS: Record<string, ModifierDisplay> = {
  // `ctrl` binds to Command on macOS (CommandOrControl), so it renders as ⌘ there.
  ctrl: { mac: '⌘', other: 'Ctrl', macOrder: 3, otherOrder: 0 },
  control: { mac: '⌘', other: 'Ctrl', macOrder: 3, otherOrder: 0 },
  // Explicit Command/Super token also renders as ⌘ on macOS, Win/Super elsewhere.
  meta: { mac: '⌘', other: 'Win', macOrder: 3, otherOrder: 3 },
  cmd: { mac: '⌘', other: 'Win', macOrder: 3, otherOrder: 3 },
  command: { mac: '⌘', other: 'Win', macOrder: 3, otherOrder: 3 },
  super: { mac: '⌘', other: 'Super', macOrder: 3, otherOrder: 3 },
  alt: { mac: '⌥', other: 'Alt', macOrder: 1, otherOrder: 1 },
  option: { mac: '⌥', other: 'Alt', macOrder: 1, otherOrder: 1 },
  shift: { mac: '⇧', other: 'Shift', macOrder: 2, otherOrder: 2 },
};

/**
 * How a call resolves user-set key aliases.
 *
 * `aliases` exists for reactivity, not capability: Svelte components must pass
 * `$hotkeyAliases` so their label is a real dependency of the store and
 * re-renders on rename. Omit it in non-reactive code and the current snapshot
 * is used instead.
 */
export interface AliasOptions {
  /** Explicit alias map (pass `$hotkeyAliases` from components). */
  aliases?: Record<string, string>;
  /**
   * Skip aliases entirely. For the alias editor, which must show a key's real
   * name ("F15") beside the field that renames it - otherwise the row relabels
   * itself as you type and you lose track of which key you're editing.
   */
  ignoreAlias?: boolean;
}

/** Alias for a normalized key token, honoring AliasOptions. */
function resolveAlias(key: string, options?: AliasOptions): string | undefined {
  if (options?.ignoreAlias) return undefined;
  if (options?.aliases) return options.aliases[key];
  return getHotkeyAlias(key);
}

/**
 * Render a single token (modifier or key) for the current platform.
 *
 * A user-set alias (Settings > Hotkeys > Alias) wins over every built-in label,
 * including the macOS f13 -> "F5" mapping: if someone explicitly renamed the
 * key, showing them anything else contradicts what they asked for. Aliases
 * apply to non-modifier keys only - Ctrl/Alt/Shift/Cmd have universal labels
 * and are what `tokensToDisplay` sorts and dedupes on.
 */
export function formatKeyToken(token: string, options?: AliasOptions): string {
  const key = token.trim().toLowerCase();
  if (!key) return '';

  const mod = MODIFIERS[key];
  if (mod) return onMac() ? mod.mac : mod.other;

  const alias = resolveAlias(key, options);
  if (alias) return alias;

  // Non-modifier keys: friendly capitalization (matches prior behavior).
  if (key === 'space') return 'Space';
  // macOS: the dedicated Dictation (mic) key is bound/registered as F13 (we
  // remap it there at the HID layer), but it's labelled F5 on the keyboard, so
  // display it as "F5" to match what the user sees.
  if (key === 'f13' && onMac()) return 'F5';
  if (key.length === 1) return key.toUpperCase();
  if (/^f\d{1,2}$/.test(key)) return key.toUpperCase();
  return key.charAt(0).toUpperCase() + key.slice(1);
}

/**
 * Format a list of raw tokens (e.g. ["ctrl","shift","space"]) for display.
 * Modifiers are sorted into canonical platform order and deduped; non-modifier
 * key(s) keep their relative order and come last.
 */
export function tokensToDisplay(tokens: string[], options?: AliasOptions): string {
  const mac = onMac();
  const seen = new Set<string>();
  const modifiers: { display: string; weight: number }[] = [];
  const keys: string[] = [];

  for (const raw of tokens) {
    const key = raw.trim().toLowerCase();
    if (!key) continue;
    const mod = MODIFIERS[key];
    if (mod) {
      const display = mac ? mod.mac : mod.other;
      if (seen.has(display)) continue; // dedupe (e.g. ctrl + meta both -> ⌘)
      seen.add(display);
      modifiers.push({ display, weight: mac ? mod.macOrder : mod.otherOrder });
    } else {
      keys.push(formatKeyToken(key, options));
    }
  }

  modifiers.sort((a, b) => a.weight - b.weight);
  const parts = [...modifiers.map(m => m.display), ...keys];

  // macOS: Apple order (⌃ ⌥ ⇧ ⌘, Command last) with spaces for legibility (⇧ ⌘ Space).
  // Other platforms: words joined with " + " (Ctrl + Shift + Space).
  return parts.join(mac ? ' ' : ' + ');
}

/**
 * Formats a stored hotkey string for display.
 * macOS: "ctrl+shift+space" -> "⌘⇧Space". Other: "Ctrl + Shift + Space".
 */
export function formatHotkey(hotkey: string, options?: AliasOptions): string {
  return tokensToDisplay(hotkey.split('+'), options);
}
