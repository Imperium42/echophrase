// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * useMicPermission - macOS microphone TCC permission handling.
 *
 * On macOS, the OS does not show its "wants to access the microphone" prompt
 * unless the app explicitly calls `AVCaptureDevice.requestAccess(for: .audio)`.
 * cpal's CoreAudio device enumeration alone is not enough. This composable
 * wraps the three Tauri commands that bridge AVFoundation so the UI has a
 * single place to drive the flow from:
 *   - the onboarding welcome screen
 *   - app startup (best-effort prompt)
 *   - the "Microphone not detected" error path
 *
 * On non-macOS platforms every call is a cheap no-op that reports "authorized".
 */
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { isMac } from '$lib/utils/platform';

export type MicPermissionStatus = 'authorized' | 'denied' | 'restricted' | 'not_determined';

/**
 * True when the current platform requires an explicit OS-level mic permission.
 *
 * Uses navigator-based detection rather than `platformCache`: the cache is only
 * filled in after an async `get_system_info` round-trip, so anything reading it
 * during early component init (onboarding, which mounts on first paint) sees
 * `'unknown'` and silently decides macOS needs nothing. `navigator.platform` is
 * available synchronously from the first frame.
 */
export function requiresExplicitMicPermission(): boolean {
  return isMac();
}

export const micPermissionStatus = writable<MicPermissionStatus>('authorized');

async function safeInvoke<T>(cmd: string, fallback: T): Promise<T> {
  try {
    return await invoke<T>(cmd);
  } catch (e) {
    console.warn(`[useMicPermission] ${cmd} failed:`, e);
    return fallback;
  }
}

/**
 * Read the current status without prompting. Updates the store as a side-effect.
 */
export async function refreshMicPermissionStatus(): Promise<MicPermissionStatus> {
  if (!requiresExplicitMicPermission()) {
    micPermissionStatus.set('authorized');
    return 'authorized';
  }
  const status = await safeInvoke<MicPermissionStatus>(
    'get_microphone_permission_status',
    'authorized'
  );
  micPermissionStatus.set(status);
  return status;
}

/**
 * Ensure the user has been asked. If status is `not_determined`, this triggers
 * the OS permission dialog and resolves after the user decides (or after a
 * 30s safety timeout on the Rust side). If already decided, returns the
 * existing status immediately.
 */
export async function ensureMicPermission(): Promise<MicPermissionStatus> {
  if (!requiresExplicitMicPermission()) {
    micPermissionStatus.set('authorized');
    return 'authorized';
  }
  const status = await safeInvoke<MicPermissionStatus>(
    'request_microphone_permission',
    'not_determined'
  );
  micPermissionStatus.set(status);
  return status;
}

/**
 * Open System Settings > Privacy & Security > Microphone so the user can flip
 * the toggle for Echophrase. macOS only.
 */
export async function openMicSystemSettings(): Promise<void> {
  if (!requiresExplicitMicPermission()) return;
  await safeInvoke<void>('open_microphone_system_settings', undefined as unknown as void);
}

/** Convenience: current status snapshot without awaiting. */
export function currentMicPermissionStatus(): MicPermissionStatus {
  return get(micPermissionStatus);
}
