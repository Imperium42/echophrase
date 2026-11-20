// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import type { TranscriptionResult, TranscriptionState } from '$lib/types/transcription';
import { defaultState } from '$lib/types/transcription';

/**
 * Creates transcription action methods for store
 */
export function createTranscriptionActions(
  update: (fn: (state: TranscriptionState) => TranscriptionState) => void,
  set: (state: TranscriptionState) => void
) {
  return {
    /** Start recording audio */
    startRecording: () => {
      update(state => ({
        ...state,
        isRecording: true,
        error: null,
        progress: 0,
      }));
    },

    /** Stop recording audio */
    stopRecording: () => {
      update(state => ({
        ...state,
        isRecording: false,
      }));
    },

    /** Start transcription process */
    startTranscription: () => {
      update(state => ({
        ...state,
        isTranscribing: true,
        error: null,
        progress: 0,
      }));
    },

    /** Update transcription progress (0-100) */
    updateProgress: (progress: number) => {
      update(state => ({
        ...state,
        progress: Math.min(100, Math.max(0, progress)),
      }));
    },

    /** Complete transcription with result */
    completeTranscription: (result: TranscriptionResult) => {
      update(state => ({
        ...state,
        isTranscribing: false,
        currentResult: result,
        history: [result, ...state.history],
        progress: 100,
      }));
    },

    /** Set error state */
    setError: (error: string) => {
      update(state => ({
        ...state,
        isRecording: false,
        isTranscribing: false,
        error,
        progress: 0,
      }));
    },

    /** Clear error state */
    clearError: () => {
      update(state => ({
        ...state,
        error: null,
      }));
    },

    /** Clear current transcription result */
    clearCurrentResult: () => {
      update(state => ({
        ...state,
        currentResult: null,
        progress: 0,
      }));
    },

    /** Remove specific result from history */
    removeFromHistory: (id: string) => {
      update(state => ({
        ...state,
        history: state.history.filter(item => item.id !== id),
      }));
    },

    /** Clear all transcription history */
    clearHistory: () => {
      update(state => ({
        ...state,
        history: [],
      }));
    },

    /** Reset to default state */
    reset: () => {
      set(defaultState);
    },
  };
}
