// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Telemetry IPC helpers
 *
 * On a fresh install the Rust side deliberately sends NOTHING at startup: the
 * first-run event trio (first_launch / app_updated / daily_active) is deferred
 * until onboarding has persisted the user's consent choice. `reportFirstRun`
 * is that release valve - call it AFTER writing `telemetry_enabled`.
 *
 * Idempotent on the Rust side (marker file + 24h window), so calling it more
 * than once is harmless.
 */

import { safeInvoke } from './invoke';

/**
 * safeInvoke, not mustInvoke: a failed analytics ping must never block the user
 * from advancing past the welcome step.
 */
export async function reportFirstRun(): Promise<void> {
  await safeInvoke<void>('telemetry_report_first_run');
}

let officialBuildPromise: Promise<boolean> | null = null;

/**
 * True only for official release builds (Rust `official` feature), the only
 * builds that can send telemetry. Community/source builds hide the consent UI.
 * Cached: the answer is fixed at compile time.
 */
export function isOfficialBuild(): Promise<boolean> {
  officialBuildPromise ??= safeInvoke<boolean>('is_official_build').then(v => v ?? false);
  return officialBuildPromise;
}
