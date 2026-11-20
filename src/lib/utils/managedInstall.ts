// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Managed-install detection.
 *
 * When a package manager (Scoop, winget, Homebrew, ...) owns the install, the
 * bundled auto-updater must stay out of the way: the manager tracks the
 * installed version, and a self-update would desync it (and on Scoop would be
 * clobbered on the next `scoop update`). The packaging step drops a
 * `.managed-install` marker next to the executable; the Rust side reads it.
 *
 * Detection fails open: anything unexpected reports "unmanaged" so ordinary
 * users keep their normal upgrade path.
 */

/** How the running build was installed. */
export interface ManagedInstall {
  /** True when a package manager owns this install and self-update is suppressed. */
  managed: boolean;
  /** Manager id from the marker file (`scoop`, `winget`, ...), if known. */
  manager: string | null;
  /** Shell command that upgrades Echophrase through `manager`, if known. */
  upgradeCommand: string | null;
}

const UNMANAGED: ManagedInstall = { managed: false, manager: null, upgradeCommand: null };

let cached: ManagedInstall | null = null;

/**
 * Resolves how this build was installed, caching the result for the session
 * (the marker cannot change while the app is running).
 */
export async function getManagedInstall(): Promise<ManagedInstall> {
  if (cached) return cached;

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const raw = await invoke<{
      managed: boolean;
      manager: string | null;
      upgrade_command: string | null;
    }>('get_managed_install');

    cached = {
      managed: Boolean(raw?.managed),
      manager: raw?.manager ?? null,
      upgradeCommand: raw?.upgrade_command ?? null,
    };
  } catch {
    // Web mode, or the command is unavailable: treat as a normal install.
    cached = UNMANAGED;
  }

  return cached;
}

/** Convenience predicate for the common "should the updater run?" check. */
export async function isManagedInstall(): Promise<boolean> {
  return (await getManagedInstall()).managed;
}
