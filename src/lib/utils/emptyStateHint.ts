// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Visibility rule for the hint under the record button (EmptyStateHint.svelte).
 *
 * Pure so it can be unit-tested without a DOM. The hint is per-session: it
 * shows on every launch and hides after the first transcription of that
 * session (the in-memory transcription history is the signal, nothing is
 * persisted).
 */

export interface EmptyStateHintInput {
  isRecording: boolean;
  isProcessing: boolean;
  isPreloading: boolean;
  onboardingComplete: boolean;
  hasTranscribedThisSession: boolean;
  modelExists: boolean;
  /** False until the hotkey store has loaded the user's real binding from settings. */
  hotkeyReady: boolean;
}

export type EmptyStateHintKind = 'none' | 'downloadModel' | 'pressToStart';

/** Decide which hint (if any) to render. Check order matters: earlier wins. */
export function resolveEmptyStateHint(input: EmptyStateHintInput): EmptyStateHintKind {
  if (input.isRecording || input.isProcessing) return 'none';
  if (input.isPreloading) return 'none';
  if (!input.onboardingComplete) return 'none';
  if (input.hasTranscribedThisSession) return 'none';
  if (!input.modelExists) return 'downloadModel';
  // Never flash the sync DEFAULT_HOTKEY placeholder before async init resolves.
  if (!input.hotkeyReady) return 'none';
  return 'pressToStart';
}
