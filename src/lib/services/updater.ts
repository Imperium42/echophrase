// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Auto-Update Service
 *
 * Handles checking for and installing app updates via CrabNebula Cloud.
 * Uses Tauri's updater plugin with signed updates for security.
 */

import { check, type Update } from '@tauri-apps/plugin-updater';
import { ask } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import { translate } from '$lib/locales';

/** Track if we've already shown an update prompt this session */
let updatePromptShown = false;

/**
 * Check for available updates
 *
 * @param silent - If true, won't show "up to date" message or prompt user
 * @returns true if update was found (and optionally installed)
 */
export async function checkForUpdates(silent = false): Promise<boolean> {
  try {
    const update = await check();

    if (!update?.available) {
      if (!silent) {
        console.log('[Updater] App is up to date');
      }
      return false;
    }

    console.log(`[Updater] Update available: ${update.version}`);

    // In silent mode, just return that update exists (don't prompt)
    if (silent && updatePromptShown) {
      return true;
    }

    // Prompt user to update
    const confirmed = await promptForUpdate(update);

    if (confirmed) {
      await installUpdate(update);
    }

    updatePromptShown = true;
    return true;
  } catch (error) {
    // Don't spam console in silent mode (startup check)
    if (!silent) {
      console.error('[Updater] Check failed:', error);
    }
    return false;
  }
}

/**
 * Show update dialog to user
 */
async function promptForUpdate(update: Update): Promise<boolean> {
  const releaseNotes = update.body || translate('update.defaultNotes');

  return await ask(
    translate('update.versionAvailable', { version: update.version, notes: releaseNotes }),
    {
      title: translate('update.updateAvailable'),
      kind: 'info',
      okLabel: translate('update.updateNow'),
      cancelLabel: translate('update.later'),
    }
  );
}

/**
 * Download and install update, then relaunch app
 */
async function installUpdate(update: Update): Promise<void> {
  console.log('[Updater] Downloading and installing...');

  // TODO: Could add progress tracking here via update.downloadAndInstall callbacks
  await update.downloadAndInstall();

  console.log('[Updater] Update installed, relaunching...');
  await relaunch();
}

/**
 * Get current app version
 */
export async function getCurrentVersion(): Promise<string> {
  try {
    const { getVersion } = await import('@tauri-apps/api/app');
    return await getVersion();
  } catch {
    return 'unknown';
  }
}
