// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Launch URL utility - platform-specific browser opening
 *
 * Uses platformCache (initialized at app startup) for instant platform detection.
 * No IPC round-trips at call time.
 *
 * - WSL: cmd.exe /c start (opens in Windows browser)
 * - Windows native: ShellExecuteW API (no console window)
 * - macOS: `open` command
 * - Linux native: xdg-open
 */

import { safeInvoke } from '$lib/ipc/invoke';
import { platformCache } from '$lib/stores/platformCache';

/**
 * Open a URL in the system's default browser.
 * Platform is detected from cache (no IPC needed).
 */
export async function launchUrl(url: string): Promise<void> {
  if (typeof window === 'undefined' || !(window as any).__TAURI_INTERNALS__) {
    // Web mode fallback
    window.open(url, '_blank');
    return;
  }

  const platform = platformCache.platform;

  switch (platform) {
    case 'wsl':
      await safeInvoke<void>('open_url_wsl', { url });
      break;
    case 'windows':
      await safeInvoke<void>('open_url_windows', { url });
      break;
    case 'macos':
      await safeInvoke<void>('open_url_mac', { url });
      break;
    case 'linux':
      await safeInvoke<void>('open_url_linux', { url });
      break;
    default:
      // Unknown platform - try xdg-open as last resort
      await safeInvoke<void>('open_url_linux', { url });
      break;
  }
}
