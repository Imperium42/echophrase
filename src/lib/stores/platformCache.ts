// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Platform Cache - Single source of truth for platform detection
 *
 * Initializes once during app startup (via useAppInit -> loadGpuContext -> get_system_info)
 * and provides synchronous access thereafter. Eliminates repeated IPC calls to detect
 * WSL/OS across components.
 *
 * Usage:
 *   import { platformCache } from '$lib/stores/platformCache';
 *   const isWsl = platformCache.isWsl;       // sync, no await
 *   const platform = platformCache.platform;  // 'wsl' | 'windows' | 'macos' | 'linux' | 'unknown'
 */

import { writable } from 'svelte/store';

export type Platform = 'wsl' | 'windows' | 'macos' | 'linux' | 'unknown';

let _isWsl = false;
let _os = 'Unknown';
let _cpuArch = 'unknown';
let _linuxDistro: string | null = null;
let _linuxDistroVersion: string | null = null;
let _initialized = false;

/** Reactive store for unsupported Mac detection (subscribable in Svelte components) */
export const isUnsupportedMac = writable(false);

/**
 * Initialize the platform cache. Called once during app startup from useAppInit
 * after loadGpuContext() resolves (which already calls get_system_info).
 */
export function initPlatformCache(
  os: string,
  isWsl: boolean,
  cpuArch: string,
  linuxDistro?: string | null,
  linuxDistroVersion?: string | null
): void {
  _os = os;
  _isWsl = isWsl;
  _cpuArch = cpuArch;
  _linuxDistro = linuxDistro ?? null;
  _linuxDistroVersion = linuxDistroVersion ?? null;
  _initialized = true;
  isUnsupportedMac.set(_os.toLowerCase().includes('mac') && _cpuArch === 'x86_64');
}

export const platformCache = {
  /** Whether we're running in WSL */
  get isWsl(): boolean {
    return _isWsl;
  },

  /** Raw OS string from backend (e.g., "Linux", "Windows", "macOS") */
  get os(): string {
    return _os;
  },

  /** CPU architecture: "x86_64" or "aarch64" */
  get cpuArch(): string {
    return _cpuArch;
  },

  /** Linux distro name mapped to NVIDIA naming (eg "Ubuntu", "Debian") */
  get linuxDistro(): string | null {
    return _linuxDistro;
  },

  /** Linux distro version (eg "24.04") */
  get linuxDistroVersion(): string | null {
    return _linuxDistroVersion;
  },

  /** Whether this is an unsupported Mac (macOS + Intel x86_64) */
  get isUnsupportedMac(): boolean {
    return _os.toLowerCase().includes('mac') && _cpuArch === 'x86_64';
  },

  /** Whether the cache has been initialized */
  get initialized(): boolean {
    return _initialized;
  },

  /** Platform identifier for auth URLs and branching logic */
  get platform(): Platform {
    if (_isWsl) return 'wsl';
    const osLower = _os.toLowerCase();
    if (osLower.includes('win')) return 'windows';
    if (osLower.includes('mac')) return 'macos';
    if (osLower.includes('linux')) return 'linux';
    return 'unknown';
  },
};
