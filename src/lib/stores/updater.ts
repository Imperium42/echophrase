// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Updater Store
 *
 * Manages app update state with reactive Svelte stores.
 * Handles checking, downloading, and installing updates via CrabNebula Cloud.
 */

import { writable, derived, get } from 'svelte/store';

/** Update check state */
export type UpdateStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error';

interface UpdateState {
  status: UpdateStatus;
  /** Available version (if any) */
  version: string | null;
  /** Release notes */
  releaseNotes: string | null;
  /** Error message (if status === 'error') */
  error: string | null;
  /** Download progress (0-100) */
  progress: number;
  /** Current app version */
  currentVersion: string;
}

const initialState: UpdateState = {
  status: 'idle',
  version: null,
  releaseNotes: null,
  error: null,
  progress: 0,
  currentVersion: '',
};

// Internal writable store
const state = writable<UpdateState>(initialState);

// Public derived stores for components
export const updateStatus = derived(state, $s => $s.status);
export const updateVersion = derived(state, $s => $s.version);
export const updateReleaseNotes = derived(state, $s => $s.releaseNotes);
export const updateError = derived(state, $s => $s.error);
export const updateProgress = derived(state, $s => $s.progress);
export const currentVersion = derived(state, $s => $s.currentVersion);

// Convenience derived stores
export const isUpdateAvailable = derived(state, $s => $s.status === 'available');
export const isChecking = derived(state, $s => $s.status === 'checking');
export const isDownloading = derived(state, $s => $s.status === 'downloading');

// Track if we've shown update prompt this session
let updatePromptShown = false;

/** Whether to show the custom update modal (set to true when prompt is triggered) */
export const showUpdateModal = writable(false);

/**
 * Set when the manual check finds a newer build on a package-manager install
 * (Homebrew/Scoop/winget). The app never self-updates there, so About shows the
 * command that upgrades it instead of a misleading "Latest".
 */
export const managedUpgrade = writable<{ version: string; command: string | null } | null>(null);

/** Open the update modal (used by UpdateBanner). */
export function openUpdateModal(): void {
  showUpdateModal.set(true);
}

/**
 * Open the modal to show release notes for the current version.
 * Used by VersionIndicator and AboutSection.
 */
export function openReleaseNotes(): void {
  const current = get(state);
  state.update(s => ({
    ...s,
    version: s.currentVersion || s.version,
    releaseNotes:
      s.releaseNotes ||
      `Release notes for v${current.currentVersion || 'current'} are not bundled in this build.`,
  }));
  showUpdateModal.set(true);
}

/**
 * Dev-only: populate a mock "update available" state and open the modal.
 * No-op in production builds.
 */
export function debugShowMockUpdate(): void {
  if (!import.meta.env.DEV) return;
  state.update(s => ({
    ...s,
    status: 'available',
    version: '99.0.0-mock',
    releaseNotes:
      '## Mock Update\n\n- Simulated update for UI testing\n- Click **Later** to dismiss',
    error: null,
  }));
  showUpdateModal.set(true);
}

/**
 * Initialize the updater store
 * Loads current app version
 */
export async function initUpdater(): Promise<void> {
  try {
    const { getVersion } = await import('@tauri-apps/api/app');
    const version = await getVersion();
    state.update(s => ({ ...s, currentVersion: version }));
  } catch {
    // Not in Tauri context
  }
}

/**
 * Check for available updates
 *
 * @param silent - If true, won't prompt user or set error state
 * @returns true if an update is available, false if up to date (or the check was
 *   skipped), null if the check itself failed (network, bad manifest, ...)
 */
export async function checkForUpdates(silent = false): Promise<boolean | null> {
  // Skip if already checking
  if (get(state).status === 'checking' || get(state).status === 'downloading') {
    return false;
  }

  // Package-manager installs (Scoop/winget/brew) are upgraded by the manager.
  // Guarding here - not just at the startup call site - also covers the manual
  // Settings > About button, which would otherwise overwrite the managed files.
  const { getManagedInstall } = await import('$lib/utils/managedInstall');
  const managed = await getManagedInstall();
  managedUpgrade.set(null);
  if (managed.managed) {
    state.update(s => ({ ...s, status: 'idle' }));
    // Startup stays quiet. The manual button does a READ-ONLY lookup (nothing is
    // downloaded or installed) so it never claims "Latest" while a newer build
    // exists; the user is told the package-manager command instead.
    if (silent) return false;
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      if (!update?.available) return false;
      managedUpgrade.set({ version: update.version, command: managed.upgradeCommand });
      return true;
    } catch (error) {
      console.error('[Updater] Managed-install check failed:', error);
      return null;
    }
  }

  state.update(s => ({ ...s, status: 'checking', error: null }));

  try {
    const { check } = await import('@tauri-apps/plugin-updater');
    const update = await check();

    if (!update?.available) {
      state.update(s => ({ ...s, status: 'idle' }));
      if (!silent) {
        console.log('[Updater] App is up to date');
      }
      return false;
    }

    console.log(`[Updater] Update available: ${update.version}`);

    state.update(s => ({
      ...s,
      status: 'available',
      version: update.version,
      releaseNotes: update.body || 'Bug fixes and improvements.',
    }));

    // In silent mode on startup, just note the update exists
    if (silent && !updatePromptShown) {
      return true;
    }

    // Prompt user to update
    if (!silent) {
      const confirmed = await promptForUpdate(update.version, update.body || null);
      if (confirmed) {
        await downloadAndInstall();
      }
      updatePromptShown = true;
    }

    return true;
  } catch (error) {
    const raw = error instanceof Error ? error.message : String(error);
    const isNoRelease = /release JSON|status code/.test(raw);
    const message = isNoRelease
      ? `${raw} (likely no public release yet - first stable build hasn't shipped)`
      : raw;
    if (!silent) {
      console.error('[Updater] Check failed:', message);
      state.update(s => ({ ...s, status: 'error', error: message }));
    } else {
      if (isNoRelease) {
        console.log('[Updater] No release found (pre-release build) - skipping');
      }
      state.update(s => ({ ...s, status: 'idle' }));
    }
    return null;
  }
}

/**
 * Show custom modal to prompt for update.
 * Returns a promise that resolves when the user clicks Update Now (true) or Later (false).
 */
async function promptForUpdate(_version: string, _releaseNotes: string | null): Promise<boolean> {
  return new Promise(resolve => {
    showUpdateModal.set(true);
    const unsub = showUpdateModal.subscribe(open => {
      // Resolved externally via resolveUpdatePrompt
      if (!open) {
        unsub();
      }
    });
    // Store resolver so modal buttons can call it
    _updatePromptResolver = resolve;
  });
}

let _updatePromptResolver: ((confirmed: boolean) => void) | null = null;

/** Called by UpdateModal buttons */
export function resolveUpdatePrompt(confirmed: boolean): void {
  showUpdateModal.set(false);
  if (_updatePromptResolver) {
    _updatePromptResolver(confirmed);
    _updatePromptResolver = null;
  }
}

/**
 * Download and install the available update
 */
export async function downloadAndInstall(): Promise<void> {
  // Same guard as checkForUpdates(): never write over a package-manager install.
  const { isManagedInstall } = await import('$lib/utils/managedInstall');
  if (await isManagedInstall()) {
    console.log('[Updater] Managed install; refusing to self-install an update.');
    state.update(s => ({ ...s, status: 'idle' }));
    return;
  }

  state.update(s => ({ ...s, status: 'downloading', progress: 0 }));

  try {
    const { check } = await import('@tauri-apps/plugin-updater');
    const update = await check();

    if (!update?.available) {
      state.update(s => ({ ...s, status: 'idle' }));
      return;
    }

    console.log('[Updater] Downloading and installing...');

    // TODO: Could add progress tracking via update.downloadAndInstall callbacks
    await update.downloadAndInstall();

    state.update(s => ({ ...s, status: 'ready', progress: 100 }));

    console.log('[Updater] Update installed, relaunching...');
    const { relaunch } = await import('@tauri-apps/plugin-process');
    await relaunch();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error('[Updater] Download failed:', message);
    state.update(s => ({ ...s, status: 'error', error: message }));
  }
}

/**
 * Clear error state and return to idle
 */
export function clearUpdateError(): void {
  state.update(s => ({ ...s, status: 'idle', error: null }));
}
