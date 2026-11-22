// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Release History Service
 *
 * Fetches prior releases via a Rust Tauri command (which calls the GitHub API
 * internally). Results are
 * cached in memory for the session as a second layer on top of the Rust TTL cache.
 * Failures are silenced - the archive section simply stays hidden.
 */

import { invoke } from '@tauri-apps/api/core';

export interface ArchiveRelease {
  version: string;
  date: string;
  body: string;
}

let cachedArchive: ArchiveRelease[] | null = null;
let fetchAttempted = false;

/**
 * Fetch up to 5 prior releases (skipping the latest) via the Rust backend.
 * Returns an empty array on any failure.
 */
export async function fetchReleaseArchive(): Promise<ArchiveRelease[]> {
  if (fetchAttempted) {
    return cachedArchive ?? [];
  }
  fetchAttempted = true;

  try {
    const archive = await invoke<ArchiveRelease[]>('fetch_release_archive');
    cachedArchive = archive;
    return archive;
  } catch {
    return [];
  }
}
