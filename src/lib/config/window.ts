// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Default main-window dimensions. Mirrors src-tauri/tauri.conf.json5
 * (app.windows[0].width/height). Used by "Reset Size" in Customize settings.
 */
export const DEFAULT_WINDOW_SIZE = {
  width: 650,
  height: 720,
} as const;
