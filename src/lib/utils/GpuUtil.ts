// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { mustInvoke, isTauriAvailable } from '$lib/ipc/invoke';
export interface SystemInfo {
  os: string;
  is_wsl: boolean;
  gpu_name: string | null;
  gpu_vram_mb: number | null;
  cpu_arch: string;
  linux_distro: string | null;
  linux_distro_version: string | null;
}

export interface GpuInfo {
  name: string | null;
  vramGb: string | null;
  vramMb: number | null;
  displayString: string | null;
}

export interface ModelRecommendation {
  model_id: string;
  vram_mb: number | null;
  vram_tier: string;
  reason: string;
  was_auto_detected: boolean;
}

/** OS info extracted from system info */
export interface OsInfo {
  os: string;
  isWsl: boolean;
  /** Display string like "Windows" or "Linux (WSL)" */
  displayString: string;
  /** CPU architecture: "x86_64" or "aarch64" */
  cpuArch: string;
  /** Linux distro name mapped to NVIDIA naming (eg "Ubuntu", "Debian") */
  linuxDistro: string | null;
  /** Linux distro version (eg "24.04") */
  linuxDistroVersion: string | null;
}

/** Combined GPU info and recommendation - single source of truth */
export interface GpuContext {
  gpuInfo: GpuInfo;
  osInfo: OsInfo;
  recommendation: ModelRecommendation | null;
  /** Whether ONNX Runtime can load on this system. False on macOS < 13.3. */
  ortAvailable: boolean;
}

// Singleton cache for GPU context (loaded once, never changes at runtime)
let cachedGpuContext: GpuContext | null = null;
let loadingPromise: Promise<GpuContext> | null = null;

const DEFAULT_GPU_INFO: GpuInfo = { name: null, vramGb: null, vramMb: null, displayString: null };
const DEFAULT_OS_INFO: OsInfo = {
  os: 'Unknown',
  isWsl: false,
  displayString: 'Unknown',
  cpuArch: 'unknown',
  linuxDistro: null,
  linuxDistroVersion: null,
};

/**
 * Load GPU context (info + recommendation) from Tauri backend.
 * Cached after first load - GPU doesn't change at runtime.
 */
export async function loadGpuContext(): Promise<GpuContext> {
  // Return cached value if available
  if (cachedGpuContext) {
    return cachedGpuContext;
  }

  // If already loading, wait for it
  if (loadingPromise) {
    return loadingPromise;
  }

  if (!isTauriAvailable()) {
    return {
      gpuInfo: DEFAULT_GPU_INFO,
      osInfo: DEFAULT_OS_INFO,
      recommendation: null,
      ortAvailable: true,
    };
  }

  // Start loading and cache the promise to prevent duplicate calls
  loadingPromise = (async () => {
    try {
      // Load all in parallel
      const [sysInfo, recommendation, ortAvailable] = await Promise.all([
        mustInvoke<SystemInfo>('get_system_info'),
        mustInvoke<ModelRecommendation>('get_model_recommendation'),
        mustInvoke<boolean>('check_ort_available'),
      ]);

      const name = sysInfo.gpu_name;
      const vramMb = sysInfo.gpu_vram_mb;
      const vramGb = vramMb ? (vramMb / 1024).toFixed(0) : null;
      const gpuDisplayString = name && vramGb ? `${name} (${vramGb}GB)` : null;

      // Build OS display string (e.g., "Windows", "Linux (WSL)", "macOS")
      const osDisplayString = sysInfo.is_wsl ? `${sysInfo.os} (WSL)` : sysInfo.os;

      cachedGpuContext = {
        gpuInfo: { name, vramGb, vramMb, displayString: gpuDisplayString },
        osInfo: {
          os: sysInfo.os,
          isWsl: sysInfo.is_wsl,
          displayString: osDisplayString,
          cpuArch: sysInfo.cpu_arch,
          linuxDistro: sysInfo.linux_distro,
          linuxDistroVersion: sysInfo.linux_distro_version,
        },
        recommendation,
        ortAvailable,
      };
      return cachedGpuContext;
    } catch (err) {
      console.warn('Failed to get GPU context:', err);
      cachedGpuContext = {
        gpuInfo: DEFAULT_GPU_INFO,
        osInfo: DEFAULT_OS_INFO,
        recommendation: null,
        ortAvailable: true,
      };
      return cachedGpuContext;
    } finally {
      loadingPromise = null;
    }
  })();

  return loadingPromise;
}

/**
 * Load system/GPU info from Tauri backend.
 * @deprecated Use loadGpuContext instead for full context including recommendations
 */
export async function loadGpuInfo(): Promise<GpuInfo> {
  const context = await loadGpuContext();
  return context.gpuInfo;
}

/**
 * Format GPU info for debug output (with backticks)
 */
export function formatGpuDebug(gpuInfo: GpuInfo): string[] {
  return [
    `GPU: \`${gpuInfo.name || 'N/A'}\``,
    `VRAM: \`${gpuInfo.vramGb ? `${gpuInfo.vramGb}GB` : 'N/A'}\``,
  ];
}
