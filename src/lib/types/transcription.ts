// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Transcription types and state definitions
 */

/** Individual transcription result */
export interface TranscriptionResult {
  id: string;
  text: string;
  timestamp: Date;
  audioFile?: string;
  language: string;
  model: string;
  duration?: number;
  confidence?: number;
}

/** Overall transcription application state */
export interface TranscriptionState {
  isRecording: boolean;
  isTranscribing: boolean;
  currentResult: TranscriptionResult | null;
  history: TranscriptionResult[];
  error: string | null;
  progress: number; // 0-100
}

/** Default transcription state */
export const defaultState: TranscriptionState = {
  isRecording: false,
  isTranscribing: false,
  currentResult: null,
  history: [],
  error: null,
  progress: 0,
};
