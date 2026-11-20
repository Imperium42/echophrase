// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Window sizing helpers (Tauri only).
 *
 * The default window size scales with the UI zoom: higher scaling means a
 * proportionally larger default window (width AND height), so scaled-up
 * content keeps fitting.
 */
import { DEFAULT_WINDOW_SIZE } from '$lib/config/window';
import { devWarn } from '$lib/utils/devLog';

/**
 * Resize the window to the default size multiplied by the given UI zoom
 * factor, then re-center it. Used by the Accessibility "Reset" action.
 */
export async function resetWindowSize(zoom = 1): Promise<void> {
  try {
    const { getCurrentWindow, LogicalSize } = await import('@tauri-apps/api/window');
    const win = getCurrentWindow();
    await win.setSize(
      new LogicalSize(DEFAULT_WINDOW_SIZE.width * zoom, DEFAULT_WINDOW_SIZE.height * zoom)
    );
    await win.center();
  } catch (e) {
    devWarn('[windowSize] resetWindowSize failed', e);
  }
}
