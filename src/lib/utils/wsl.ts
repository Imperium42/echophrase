// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * WSL detection helpers
 *
 * Uses the platform cache that is populated at app startup.
 * Avoids repeated IPC round-trips for environment detection.
 */

import { platformCache } from '$lib/stores/platformCache';

/** Returns true when running inside a WSL environment. */
export function checkWslEnvironment(): boolean {
  return platformCache.isWsl;
}
