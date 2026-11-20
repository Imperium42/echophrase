// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Recording-specific IPC invoke wrapper.
 *
 * Extends the base IPC layer with retry logic, per-command timeouts,
 * and auto-restart on repeated connection failures. Used exclusively by
 * recording composables - not a general-purpose helper.
 */

import { translate } from '$lib/locales';
import { isTauriAvailable } from '$lib/ipc/invoke';
import { get } from 'svelte/store';
import { executionProviderStore } from '$lib/stores/executionProvider';

// Transcription commands are long-running and non-idempotent - never retry.
const NO_RETRY_COMMANDS = [
  'transcribe_audio',
  'stop_audio_recording_and_transcribe',
  'transcribe_open_mic_chunk',
] as const;

/** Floor/ceiling constants for transcription timeout. */
const TRANSCRIPTION_TIMEOUT_FLOOR_MS = 60_000;
const TRANSCRIPTION_TIMEOUT_CEILING_MS = 1_800_000;

/** Fallback audio duration (seconds) used when callers do not supply audio_duration_sec. */
const TRANSCRIPTION_FALLBACK_AUDIO_SEC = 300;

/**
 * Compute a dynamic transcription timeout based on audio length and execution provider.
 *
 * Formula:
 *   base = 60s (model load + startup overhead)
 *   first_run_penalty = 120s (DirectML shader JIT grace period)
 *   realtime_factor = 0.5 (cuda) | 1.0 (directml) | 0.8 (coreml) | 6.0 (cpu)
 *   timeout_sec = base + first_run_penalty + audio_sec * realtime_factor
 *
 * Floor: 60s. Ceiling: 1800s (30 min).
 */
export function computeTranscriptionTimeoutMs(audioSec: number, ep: string): number {
  const baseSec = 60;
  const firstRunPenaltySec = 120;
  // Normalize: backend may report 'direct_ml', 'directml', 'DirectML', 'core_ml', etc.
  const epNorm = ep.toLowerCase().replace(/[_\s-]/g, '');
  const realtimeFactor =
    epNorm === 'cuda'
      ? 0.5
      : epNorm === 'directml'
        ? 1.0
        : epNorm === 'coreml'
          ? 0.8
          : /* cpu fallback */ 6.0;
  const timeoutSec = baseSec + firstRunPenaltySec + audioSec * realtimeFactor;
  const clampedSec = Math.max(
    TRANSCRIPTION_TIMEOUT_FLOOR_MS / 1000,
    Math.min(TRANSCRIPTION_TIMEOUT_CEILING_MS / 1000, timeoutSec)
  );
  return clampedSec * 1000;
}

// Timeout for all other commands.
const DEFAULT_COMMAND_TIMEOUT_MS = 30_000;

const MAX_RETRIES = 3;
const RETRY_DELAYS_MS = [1000, 2000, 4000] as const;

// Auto-restart tracking - module-level to survive composable re-instantiation.
let autoRestartAttempted = false;

/** Trigger an application relaunch via the Tauri process plugin. */
async function triggerAppRestart(): Promise<void> {
  if (autoRestartAttempted) {
    console.log('Auto-restart already attempted, skipping');
    return;
  }
  autoRestartAttempted = true;

  try {
    console.log('Triggering automatic app restart...');
    const { relaunch } = await import('@tauri-apps/plugin-process');
    await relaunch();
  } catch (error) {
    console.error('Failed to auto-restart:', error);
    autoRestartAttempted = false;
  }
}

/** Raw Tauri invoke handle - loaded lazily alongside the mic recorder APIs. */
let invokeHandle: ((cmd: string, args?: unknown) => Promise<unknown>) | null = null;

/** Inject the resolved invoke handle (called from useRecording after API init). */
export function setInvokeHandle(fn: (cmd: string, args?: unknown) => Promise<unknown>): void {
  invokeHandle = fn;
}

/**
 * Invoke a Tauri command with timeout, retry, and auto-restart semantics.
 *
 * - Transcription commands: one-shot with a dynamic timeout based on audio length and EP.
 *   Pass `audio_duration_sec` in `args` for accurate timeout; omitting it falls back to
 *   a 300s default (TRANSCRIPTION_FALLBACK_AUDIO_SEC) and logs a warning.
 *   Supply `timeoutMs` to override the computed value entirely.
 * - Other commands: up to 3 retries with exponential backoff, then auto-restart.
 */
export async function recordingInvoke<T>(
  command: string,
  args?: Record<string, unknown>,
  retryCount = 0,
  timeoutMs?: number
): Promise<T> {
  if (!isTauriAvailable() || !invokeHandle) {
    throw new Error(translate('errors.webModeFull'));
  }

  const isTranscriptionCmd = (NO_RETRY_COMMANDS as readonly string[]).includes(command);

  let resolvedTimeoutMs: number;
  if (typeof timeoutMs === 'number') {
    resolvedTimeoutMs = timeoutMs;
  } else if (isTranscriptionCmd) {
    const ep = get(executionProviderStore).active;
    let audioSec: number;
    if (typeof args?.audio_duration_sec === 'number') {
      audioSec = args.audio_duration_sec;
    } else {
      console.warn(
        `[recordingInvoke] "${command}" called without audio_duration_sec - ` +
          `falling back to ${TRANSCRIPTION_FALLBACK_AUDIO_SEC}s. ` +
          `Update the call site to pass audio_duration_sec for accurate timeouts.`
      );
      audioSec = TRANSCRIPTION_FALLBACK_AUDIO_SEC;
    }
    resolvedTimeoutMs = computeTranscriptionTimeoutMs(audioSec, ep);
    console.log(
      `[recordingInvoke] "${command}" timeout: ${(resolvedTimeoutMs / 1000).toFixed(0)}s ` +
        `(audio=${audioSec}s, ep=${ep})`
    );
  } else {
    resolvedTimeoutMs = DEFAULT_COMMAND_TIMEOUT_MS;
  }

  try {
    const timeoutPromise = new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error('TIMEOUT')), resolvedTimeoutMs)
    );
    const invokePromise = invokeHandle(command, args) as Promise<T>;
    return await Promise.race([invokePromise, timeoutPromise]);
  } catch (error: unknown) {
    const msg = error instanceof Error ? error.message : String(error);
    const isTimeout =
      msg === 'TIMEOUT' ||
      msg.includes('Connection refused') ||
      msg.includes('WebSocket') ||
      msg.includes('timeout');

    if (isTimeout && isTranscriptionCmd) {
      const ep = get(executionProviderStore).active;
      const timeoutSec = Math.round(resolvedTimeoutMs / 1000);
      console.error(`"${command}" timed out after ${timeoutSec}s`);
      const baseMsg = translate('errors.transcriptionTimeout', { seconds: String(timeoutSec) });
      const hint = ep === 'cpu' ? ' ' + translate('errors.transcriptionTimeoutHint') : '';
      throw new Error(baseMsg + hint);
    }

    if (isTimeout && retryCount < MAX_RETRIES) {
      const delay = RETRY_DELAYS_MS[retryCount] ?? 4000;
      console.log(`Retry ${retryCount + 1}/${MAX_RETRIES} for "${command}" in ${delay}ms...`);
      await new Promise<void>(resolve => setTimeout(resolve, delay));
      return recordingInvoke<T>(command, args, retryCount + 1, resolvedTimeoutMs);
    }

    if (isTimeout) {
      console.error(`Max retries exceeded for "${command}". Triggering auto-restart...`);
      setTimeout(() => triggerAppRestart(), 1500);
      throw new Error(translate('errors.connectionLost'));
    }

    if (
      msg.includes('command not found') ||
      msg.includes('not registered') ||
      msg.includes('failed to initialize')
    ) {
      throw new Error(translate('errors.serverInitializing'));
    }

    throw error;
  }
}
