// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Model auto-detection utilities.
 *
 * Calls into Rust backend for VRAM-based model selection on first launch.
 */

import { mustInvoke } from '$lib/ipc/invoke';
import { writable } from 'svelte/store';

/**
 * VRAM tier classification from the backend.
 */
export type VramTier = 'none' | 'insufficient' | 'low' | 'medium' | 'high' | 'premium';

/**
 * Result of model auto-detection from the Rust backend.
 */
export interface AutoDetectionResult {
  /** Selected model ID */
  model_id: string;
  /** Detected VRAM in MB (null if unknown) */
  vram_mb: number | null;
  /** VRAM tier classification */
  vram_tier: VramTier;
  /** Human-readable reason for selection */
  reason: string;
  /** Whether auto-detection was performed (vs just a recommendation) */
  was_auto_detected: boolean;
}

/**
 * Check for first launch and auto-detect optimal model if needed.
 *
 * Returns null if:
 * - User has already manually configured a model
 * - Auto-detection has already run previously
 *
 * Returns result if this is first launch and detection was performed.
 */
export async function maybeAutoDetectModel(): Promise<AutoDetectionResult | null> {
  return mustInvoke<AutoDetectionResult | null>('maybe_auto_detect_model');
}

/**
 * Force re-detection of optimal model.
 *
 * Always runs detection regardless of previous state.
 * Useful for settings reset or hardware changes.
 */
export async function forceAutoDetectModel(): Promise<AutoDetectionResult> {
  return mustInvoke<AutoDetectionResult>('force_auto_detect_model');
}

/**
 * Get model recommendation based on current GPU without changing settings.
 *
 * Useful for displaying recommendations in the UI.
 */
export async function getModelRecommendation(): Promise<AutoDetectionResult> {
  return mustInvoke<AutoDetectionResult>('get_model_recommendation');
}

/**
 * Reset model detection state to trigger re-detection on next launch.
 *
 * This clears the auto_detected_model flag and resets to default model.
 */
export async function resetModelDetection(): Promise<void> {
  return mustInvoke<void>('reset_model_detection');
}

/**
 * Format VRAM for display (converts MB to GB with 1 decimal).
 */
export function formatVramForDisplay(vramMb: number | null): string {
  if (vramMb === null) {
    return 'Unknown';
  }
  const gb = vramMb / 1024;
  return `${gb.toFixed(1)} GB`;
}

/**
 * Get a user-friendly description of the VRAM tier.
 */
export function getVramTierDescription(tier: VramTier): string {
  switch (tier) {
    case 'none':
      return 'No GPU detected';
    case 'insufficient':
      return 'Less than 2GB VRAM';
    case 'low':
      return '2-4GB VRAM';
    case 'medium':
      return '4-6GB VRAM';
    case 'high':
      return '6-8GB VRAM';
    case 'premium':
      return '8GB+ VRAM';
    default:
      return 'Unknown';
  }
}

// ============================================================================
// Model Auto-Detection Toast Store
// ============================================================================

export interface ModelAutoDetectToast {
  id: string;
  modelId: string;
  modelName: string;
  vramMb: number | null;
  reason: string;
}

const modelAutoDetectToastStore = writable<ModelAutoDetectToast | null>(null);

/** Store for model auto-detect toast - subscribe in UI */
export const modelAutoDetectToast = {
  subscribe: modelAutoDetectToastStore.subscribe,
};

/** Show model auto-detection toast */
export function showModelAutoDetectToast(result: AutoDetectionResult, modelName: string): void {
  const toast: ModelAutoDetectToast = {
    id: `auto-detect-${Date.now()}`,
    modelId: result.model_id,
    modelName,
    vramMb: result.vram_mb,
    reason: result.reason,
  };
  modelAutoDetectToastStore.set(toast);
  // Auto-dismiss after 5 seconds
  setTimeout(() => {
    modelAutoDetectToastStore.set(null);
  }, 5000);
}

/** Dismiss the model auto-detect toast */
export function dismissModelAutoDetectToast(): void {
  modelAutoDetectToastStore.set(null);
}
