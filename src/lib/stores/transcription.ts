// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable } from 'svelte/store';
import type { TranscriptionState } from '$lib/types/transcription';
import { defaultState } from '$lib/types/transcription';
import { createTranscriptionActions } from './transcriptionActions';

// Create transcription store
function createTranscriptionStore() {
  const { subscribe, set, update } = writable<TranscriptionState>(defaultState);

  return {
    subscribe,
    set,
    update,
    ...createTranscriptionActions(update, set),
  };
}

export const transcription = createTranscriptionStore();
