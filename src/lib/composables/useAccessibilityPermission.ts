// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * useAccessibilityPermission - macOS Accessibility permission handling for
 * "paste at cursor".
 *
 * Why this exists: pasting at the cursor works by synthesizing a Cmd+V
 * keystroke into whatever app the user is focused on. macOS gates that behind
 * `kTCCServicePostEvent` - the send-only TCC service, NOT the broader
 * "Accessibility" (`kTCCServiceAccessibility`) that reads other apps' UI, nor
 * "Input Monitoring" (`kTCCServiceListenEvent`) that reads every keystroke.
 * System Settings unhelpfully surfaces the send-only one under the same
 * "Accessibility" switch, labelled "control your computer".
 *
 * So we check trust ourselves (a pure query that never prompts), and when it is
 * missing we show our own modal explaining the reason before macOS ever gets a
 * chance to ask. The Rust side refuses to call enigo while untrusted, returning
 * the `ACCESSIBILITY_REQUIRED` sentinel.
 *
 * On non-macOS platforms every call is a cheap no-op reporting "granted".
 */
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { isMac } from '$lib/utils/platform';

/** Sentinel the Rust `simulate_paste` command returns when trust is missing. */
export const ACCESSIBILITY_REQUIRED = 'ACCESSIBILITY_REQUIRED';

/**
 * True when the current platform gates keystroke synthesis behind a permission.
 *
 * navigator-based for the same reason as the mic check - `platformCache` is not
 * populated until an async round-trip completes, which is too late for anything
 * that renders on first paint.
 */
export function requiresAccessibilityPermission(): boolean {
  return isMac();
}

/** Whether the app is currently trusted to synthesize keystrokes. */
export const accessibilityGranted = writable<boolean>(true);

/** Drives the explainer modal. Set when a paste is blocked by missing trust. */
export const showAccessibilityModal = writable<boolean>(false);

/** Matches an error thrown by `invoke('simulate_paste')` against the sentinel. */
export function isAccessibilityError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.includes(ACCESSIBILITY_REQUIRED);
}

/**
 * Read current trust without prompting. Updates the store as a side-effect.
 *
 * Caveat worth knowing before you build a polling loop on this: CoreGraphics
 * caches a negative answer for the life of the process, so granting the
 * permission while the app runs may keep reporting `false` until Echophrase
 * restarts. Treat `false` as "not granted yet in this process".
 */
export async function refreshAccessibilityPermission(): Promise<boolean> {
  if (!requiresAccessibilityPermission()) {
    accessibilityGranted.set(true);
    return true;
  }
  try {
    const granted = await invoke<boolean>('get_paste_permission_granted');
    accessibilityGranted.set(granted);
    return granted;
  } catch (e) {
    console.warn('[useAccessibilityPermission] status check failed:', e);
    // Fail open: never block pasting because the query itself broke.
    accessibilityGranted.set(true);
    return true;
  }
}

/**
 * Raise the macOS permission prompt for keystroke posting (the narrow
 * `kTCCServicePostEvent` request, not full Accessibility) and report the result.
 *
 * Show our own explanation before calling this - the OS dialog's wording is not
 * ours to control. Returns the post-decision state; a `false` may simply mean
 * CoreGraphics has cached the pre-grant answer for this process (see
 * `refreshAccessibilityPermission`).
 */
export async function requestPastePermission(): Promise<boolean> {
  if (!requiresAccessibilityPermission()) return true;
  try {
    const granted = await invoke<boolean>('request_paste_permission');
    accessibilityGranted.set(granted);
    return granted;
  } catch (e) {
    console.warn('[useAccessibilityPermission] request failed:', e);
    return false;
  }
}

/**
 * Open System Settings > Privacy & Security > Accessibility so the user can add
 * Echophrase to the allow-list. macOS only.
 */
export async function openAccessibilitySystemSettings(): Promise<void> {
  if (!requiresAccessibilityPermission()) return;
  try {
    await invoke('open_accessibility_system_settings');
  } catch (e) {
    console.warn('[useAccessibilityPermission] failed to open settings pane:', e);
  }
}

/** Convenience: current trust snapshot without awaiting. */
export function currentAccessibilityGranted(): boolean {
  return get(accessibilityGranted);
}
