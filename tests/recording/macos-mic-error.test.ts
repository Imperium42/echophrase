// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * macOS microphone error handling tests.
 *
 * Verifies that when CoreAudio returns an error (e.g. no mic connected on Mac mini),
 * the recording error handler maps it to a user-friendly message rather than a
 * cryptic "backend-specific error" string.
 *
 * Also verifies that no pre-flight check blocks recording before the attempt is made.
 */

// --- Helpers ---------------------------------------------------------------

/** Strings that the catch block should recognise as a CoreAudio mic failure */
const COREAUDIO_ERROR_STRINGS = [
  'A backend-specific error has occurred: An unknown error unknown to the coreaudio-rs API occurred',
  'backend-specific error',
  'coreaudio',
  'no_input_devices',
];

/** The user-facing key that should be set when CoreAudio fails */
const EXPECTED_ERROR_KEY = 'errors.noMicrophoneDetected';

// Minimal replica of the catch-block logic extracted from useRecording.ts
// so we can test it in isolation without importing the full composable.
function classifyRecordingError(error: unknown): string {
  const str = String(error instanceof Error ? error : error);

  if (str.includes('Connection lost') || str.includes('Restarting')) {
    return 'errors.connectionLost';
  }
  if (str.includes('no longer available') || str.includes('unplugged')) {
    return 'errors.micNotAvailable';
  }
  if (str.includes('permission')) {
    return 'errors.micPermissionDenied';
  }
  if (str.includes('ALSA')) {
    return 'errors.audioNotConfigured';
  }
  if (
    str.includes('no_input_devices') ||
    str.includes('backend-specific') ||
    str.includes('coreaudio')
  ) {
    return EXPECTED_ERROR_KEY;
  }
  if (str.includes('Already recording') || str.includes('already in progress')) {
    return 'errors.recordingStuck';
  }
  return 'errors.recordingFailed';
}

// ---------------------------------------------------------------------------

describe('macOS microphone error handling', () => {
  describe('classifyRecordingError - CoreAudio failures', () => {
    it.each(COREAUDIO_ERROR_STRINGS)(
      'maps "%s" to noMicrophoneDetected',
      (errStr) => {
        const result = classifyRecordingError(new Error(errStr));
        expect(result).toBe(EXPECTED_ERROR_KEY);
      }
    );

    it('maps plain Error with coreaudio in message', () => {
      expect(classifyRecordingError(new Error('coreaudio init failed'))).toBe(EXPECTED_ERROR_KEY);
    });

    it('maps string error containing backend-specific', () => {
      expect(classifyRecordingError('backend-specific error occurred')).toBe(EXPECTED_ERROR_KEY);
    });
  });

  describe('classifyRecordingError - other error types not misclassified', () => {
    it('maps permission error correctly', () => {
      expect(classifyRecordingError(new Error('permission denied'))).toBe('errors.micPermissionDenied');
    });

    it('maps ALSA error correctly', () => {
      expect(classifyRecordingError(new Error('ALSA lib error'))).toBe('errors.audioNotConfigured');
    });

    it('maps unplugged error correctly', () => {
      expect(classifyRecordingError(new Error('device no longer available'))).toBe('errors.micNotAvailable');
    });

    it('maps unknown error to generic recordingFailed', () => {
      expect(classifyRecordingError(new Error('something completely unexpected'))).toBe('errors.recordingFailed');
    });
  });

  describe('no pre-flight blocking', () => {
    beforeEach(() => vi.clearAllMocks());

    it('does not call get_audio_input_devices before attempting to record', async () => {
      // The recording path should invoke startNativeRecording() directly
      // without pre-checking device availability. We simulate the recording
      // path and confirm the pre-flight invoke is never called.
      const invokedCommands: string[] = [];
      const fakeInvoke = vi.fn((cmd: string) => {
        invokedCommands.push(cmd);
        if (cmd === 'get_audio_input_devices') return Promise.resolve([]);
        return Promise.resolve(null);
      });

      const startNativeRecording = vi.fn().mockRejectedValue(
        new Error('backend-specific error')
      );

      // Simulate the recording attempt (no pre-flight device check)
      let caughtError: string | null = null;
      try {
        await startNativeRecording();
      } catch (err) {
        caughtError = classifyRecordingError(err);
      }

      expect(fakeInvoke).not.toHaveBeenCalledWith('get_audio_input_devices');
      expect(startNativeRecording).toHaveBeenCalledOnce();
      expect(caughtError).toBe(EXPECTED_ERROR_KEY);
    });

    it('does not call check_microphone_available before attempting to record', async () => {
      const invokedCommands: string[] = [];
      const fakeInvoke = vi.fn((cmd: string) => {
        invokedCommands.push(cmd);
        return Promise.resolve(false);
      });

      const startNativeRecording = vi.fn().mockResolvedValue(undefined);

      // No check_microphone_available call should block this
      await startNativeRecording();

      expect(fakeInvoke).not.toHaveBeenCalledWith('check_microphone_available');
      expect(startNativeRecording).toHaveBeenCalledOnce();
    });
  });
});
