// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Pure utilities for filtering / matching models against the user's hardware.
 *
 * Extracted from ModelDownloadProgress.svelte so the compatibility logic is
 * unit-testable without spinning up the dialog component, and so other UIs
 * (e.g. settings auto-detect, recommendations) can reuse the same checks.
 *
 * Zero Svelte / store / IPC dependencies — pass current state in by argument.
 */

import type { ModelDefinition, ModelCategory } from '$lib/types/models';
import type { GpuInfo } from '$lib/utils/GpuUtil';

/**
 * Parse the `minVram` string on a `ModelDefinition` (e.g. `"4GB"`, `"512MB"`)
 * into megabytes. Numbers without a unit are assumed to be GB.
 */
export function parseVramMb(minVram: string): number {
  const match = minVram.match(/(\d+)/);
  if (!match) return 0;
  const value = parseInt(match[1], 10);
  // No unit / "GB" -> GB; legacy callers pass strings like "4GB" or just "4".
  return /\bMB\b/i.test(minVram) ? value : value * 1024;
}

/**
 * Whether the user's hardware can run this model.
 *
 * - ONNX models require ORT to load; on systems where ORT is unavailable
 *   (e.g. macOS < 13.3) only the `whisper-small` CPU fallback is allowed.
 * - For local models with a known `minVram`, the user's reported VRAM must
 *   meet or exceed it. If VRAM is unknown, we permit (and let the runtime
 *   surface failure rather than gate optimistically here).
 */
export function isModelCompatible(
  model: ModelDefinition,
  gpuInfo: Pick<GpuInfo, 'vramMb'>,
  ortAvailable: boolean
): boolean {
  if (!ortAvailable && model.format === 'onnx') return false;
  // whisper-small is the CPU fallback for old Macs - always compatible
  if (!ortAvailable && model.id === 'whisper-small') return true;
  if (!gpuInfo.vramMb) return true;
  return gpuInfo.vramMb >= parseVramMb(model.minVram);
}

/**
 * Tooltip copy explaining a GPU recommendation. "RTX ..." tiers get a
 * "recommended, or equivalent - lower tiers may still work" hint;
 * "Potato" tiers get category-specific copy; anything else returns `null`.
 *
 * The wording differs per category:
 * - Voice category gets the "any GPU, even CPU" framing.
 * - Transform category Older Potato emphasizes CPU as primary.
 * - Transform category Newer Potato emphasizes a GPU as primary.
 *
 * Callers pass their bound `t` so this stays locale-aware without
 * importing the locales module here.
 */
export function getGpuTooltip(
  gpuRec: string,
  category: ModelCategory,
  t: (key: string) => string
): string | null {
  if (gpuRec.startsWith('RTX')) {
    return t('models.gpuRecTierHint').replace('{gpu}', gpuRec.replace(/\+$/, ''));
  }
  if (!gpuRec.includes('Potato')) return null;
  if (category !== 'transformation') return t('models.gpuRecPotatoVoice');
  if (gpuRec === 'Newer Potato') return t('models.gpuRecPotatoTransformNewer');
  return t('models.gpuRecPotatoTransformOlder');
}
