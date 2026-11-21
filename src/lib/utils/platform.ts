// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Platform Detection Utilities
 *
 * Detects the current platform for conditional UI/behavior.
 * WSL is detected via user agent or navigator platform heuristics.
 */

/**
 * Check if running on Windows (native, not WSL)
 */
export function isWindows(): boolean {
  if (typeof navigator === 'undefined') return false;
  return navigator.platform?.startsWith('Win') ?? false;
}

/**
 * Check if running on macOS
 */
export function isMac(): boolean {
  if (typeof navigator === 'undefined') return false;
  return navigator.platform?.startsWith('Mac') ?? false;
}

/**
 * Check if running on Linux (includes WSL)
 */
export function isLinux(): boolean {
  if (typeof navigator === 'undefined') return false;
  return navigator.platform?.startsWith('Linux') ?? false;
}

/**
 * Check if running in WSL (Windows Subsystem for Linux)
 *
 * Detection methods:
 * 1. Check for WSL-specific environment indicators
 * 2. Check user agent for WSL hints
 * 3. Platform is Linux but display hints at Windows
 */
export function isWSL(): boolean {
  if (typeof navigator === 'undefined') return false;

  // If not Linux platform, can't be WSL
  if (!isLinux()) return false;

  // Check for WSL in user agent (some browsers include it)
  const ua = navigator.userAgent?.toLowerCase() ?? '';
  if (ua.includes('wsl')) return true;

  // WSL2 runs with X11/Wayland via WSLg - check for display server hints
  // This is a heuristic - in WSL, DISPLAY is usually :0 via WSLg
  // Note: This can't be detected from browser JS, would need Tauri backend

  return false;
}

/**
 * Check if running in WSL using Tauri backend detection
 * This is more reliable than browser-based detection
 */
export async function isWSLAsync(): Promise<boolean> {
  if (typeof window === 'undefined') return false;

  // Check if Tauri is available
  if (!(window as any).__TAURI_INTERNALS__) return false;

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    // Call backend to check for WSL
    const result = await invoke<boolean>('is_wsl');
    return result;
  } catch {
    // Fallback to heuristic
    return isLinux();
  }
}

/**
 * Get a human-readable platform name
 */
export function getPlatformName(): string {
  if (isWindows()) return 'Windows';
  if (isMac()) return 'macOS';
  if (isLinux()) return 'Linux';
  return 'Unknown';
}

/**
 * Check if DirectML is available on this platform
 * DirectML is Windows-only (via DirectX 12)
 */
export function isDirectMLAvailable(): boolean {
  return isWindows();
}

/**
 * Check if CoreML is available (macOS only)
 */
export function isCoreMLAvailable(): boolean {
  return isMac();
}
