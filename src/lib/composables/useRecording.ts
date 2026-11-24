// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable, get } from 'svelte/store';
import { beginRun, publishRun, type LastRunAction, type LastRunSpeed } from '$lib/stores/lastRun';
import { useOutputHandler, type OutputMode } from '$lib/composables/useOutputHandler';
import { tauriSettings, type TauriSettings } from '$lib/composables/useTauriSettings';
import { defaultSettings } from '$lib/types/settings';
import {
  removeFillerWords,
  convertNumberWordsToDigits,
  appendTrailingSpaceIfIncomplete,
  useTranscriptFormatter,
} from '$lib/composables/useTranscriptFormatter';
import { platformCache } from '$lib/stores/platformCache';
import { translate } from '$lib/locales';
import { promiseWithTimeout, TimeoutError } from '$lib/utils/promiseWithTimeout';
import { isSilenceResult } from '$lib/utils/silenceResult';
import { planOutputModes } from '$lib/utils/outputPlan';
import { devLog } from '$lib/utils/devLog';
import { addOutputToast } from '$lib/stores/outputToasts';
import type { UnlistenFn } from '@tauri-apps/api/event';
import type { InvokeArgs } from '@tauri-apps/api/core';

/** Shape we probe on caught values: Tauri rejects with plain strings as well as Errors. */
type CaughtError = { message?: string; toString?: () => string } | null | undefined;

// Speed toast duration (ms) - must match CSS animation in app.css
const SPEED_TOAST_DURATION_MS = 4000;

// Transcription timeout (ms) - if backend doesn't respond within this time,
// abort and show an error instead of spinning forever.
// CPU transcription on older hardware can be slow, so this is generous.
const TRANSCRIPTION_TIMEOUT_MS = 300_000; // 5 minutes - CPU transcription on older hardware can take 2+ min

// Open mic per-chunk transcription timeout (ms). Open mic chunks are short
// (a few seconds of speech at most), so this is much tighter than the
// standalone transcribe timeout above - it's a watchdog against a single
// stuck chunk request wedging the whole open mic loop (isProcessing never
// clears, chunks pile up in the Rust pending_queue forever).
const OPEN_MIC_CHUNK_TIMEOUT_MS = 120_000; // 2 minutes

/** The user's output modes (Settings > Output). Shared by push-to-talk and open mic. */
function resolveOutputModes(settings: TauriSettings | null): string[] {
  if (Array.isArray(settings?.output_to) && settings.output_to.length > 0) {
    return settings.output_to;
  }
  if (typeof settings?.output_to === 'string' && settings.output_to !== 'none') {
    return [settings.output_to];
  }
  console.warn('⚠️ No output_to setting found, defaulting to copy_to_clipboard');
  return ['copy_to_clipboard']; // Safe fallback - never auto-pastes
}

/** Run every configured output mode; returns the ones that succeeded. */
async function deliverOutput(
  formattedOutput: string,
  settings: TauriSettings | null,
  openMic = false
): Promise<LastRunAction[]> {
  const plan = planOutputModes(resolveOutputModes(settings), settings, openMic);
  const outputModes = plan.run;
  console.log('📤 Output modes from settings:', JSON.stringify(outputModes));

  const succeededActions: LastRunAction[] = [];
  const outputHandler = useOutputHandler();
  // Paste goes through the clipboard. By default the transcript stays there;
  // the opt-in setting restores the previous clipboard text after pasting
  // (only when Copy is off, see planOutputModes).
  const keepOnClipboard = !plan.restoreClipboard;
  for (const mode of outputModes) {
    const result = await outputHandler.handleOutput(
      formattedOutput,
      mode as OutputMode,
      undefined,
      keepOnClipboard
    );
    if (!result.success) {
      console.warn(`⚠️ Output handling failed for ${mode}: ${result.error}`);
      // Copy was folded into this paste; the paste failed, so run the copy for real.
      if (mode === 'paste_at_cursor' && plan.copyImpliedByPaste) {
        const copied = await outputHandler.handleOutput(formattedOutput, 'copy_to_clipboard');
        if (copied.success) {
          succeededActions.push('copy_to_clipboard');
          if (settings?.show_output_confirmations !== false) {
            addOutputToast('copy_to_clipboard', { text: formattedOutput });
          }
        }
      }
      continue;
    }
    if (mode === 'copy_to_clipboard' || mode === 'paste_at_cursor' || mode === 'save_as_file') {
      succeededActions.push(mode);
    }
    if (settings?.show_output_confirmations !== false) {
      addOutputToast(mode, {
        text: formattedOutput,
        filePath: (result as { savedPath?: string }).savedPath,
      });
    }
    // Copy was folded into a successful paste: the text is on the clipboard.
    if (mode === 'paste_at_cursor' && plan.copyImpliedByPaste) {
      succeededActions.push('copy_to_clipboard');
      if (settings?.show_output_confirmations !== false) {
        addOutputToast('copy_to_clipboard', { text: formattedOutput });
      }
    }
  }
  return succeededActions;
}

// Speed toast store (separate from output toasts)
export interface SpeedToast {
  id: string;
  transcriptionSeconds: number;
  transformSeconds: number | null; // null if transform not enabled
  totalSeconds: number;
}
const speedToastStore = writable<SpeedToast | null>(null);

/** Store for speed toast - subscribe in UI */
export const speedToast = {
  subscribe: speedToastStore.subscribe,
};

/** Show speed toast with transcription duration */
function showSpeedToast(transcriptionSeconds: number, transformSeconds: number | null = null) {
  const totalSeconds = transcriptionSeconds + (transformSeconds ?? 0);
  const toast: SpeedToast = {
    id: `speed-${Date.now()}`,
    transcriptionSeconds,
    transformSeconds,
    totalSeconds,
  };
  speedToastStore.set(toast);
  setTimeout(() => {
    speedToastStore.set(null);
  }, SPEED_TOAST_DURATION_MS);
}

// Dynamic Tauri API references
let invoke: typeof import('@tauri-apps/api/core').invoke | null = null;

// Check if Tauri context is available
function isTauriAvailable(): boolean {
  return typeof window !== 'undefined' && typeof window.__TAURI_INTERNALS__ !== 'undefined';
}

// Initialize Tauri APIs dynamically
async function initializeTauriAPIs() {
  if (isTauriAvailable()) {
    try {
      const core = await import('@tauri-apps/api/core');
      invoke = core.invoke;

      // Reset any stale recording state from crash/refresh (best-effort)
      try {
        await invoke('reset_recording_state');
      } catch {
        // Ignore - no recording was in progress
      }

      console.log('✅ Tauri APIs initialized:', { hasInvoke: !!invoke });
      return true;
    } catch (error) {
      console.warn('⚠️ Failed to load Tauri APIs:', error);
      return false;
    }
  }
  return false;
}

// Auto-restart tracking
let autoRestartAttempted = false;

/** Trigger app restart using Tauri process plugin */
async function triggerAppRestart(): Promise<void> {
  if (autoRestartAttempted) {
    console.log('⚠️ Auto-restart already attempted, skipping');
    return;
  }
  autoRestartAttempted = true;

  try {
    console.log('🔄 Triggering automatic app restart...');
    const { relaunch } = await import('@tauri-apps/plugin-process');
    await relaunch();
  } catch (error) {
    console.error('❌ Failed to auto-restart:', error);
    // Reset flag so user can try again
    autoRestartAttempted = false;
  }
}

// Safe invoke wrapper with auto-retry and friendly error handling
const MAX_RETRIES = 3;
const RETRY_DELAYS = [1000, 2000, 4000]; // Exponential backoff

async function safeInvoke<T>(command: string, args?: InvokeArgs, retryCount = 0): Promise<T> {
  if (!isTauriAvailable() || !invoke) {
    throw new Error(translate('errors.webModeFull'));
  }

  try {
    // Add timeout wrapper - 5 min for transcription operations, 30 seconds for others
    const timeoutDuration =
      command === 'transcribe_audio' ||
      command === 'stop_audio_recording_and_transcribe' ||
      command === 'transcribe_open_mic_chunk'
        ? TRANSCRIPTION_TIMEOUT_MS // 5 minutes for CPU transcription on older hardware
        : 30000; // 30 seconds for other operations

    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error('TIMEOUT')), timeoutDuration);
    });

    const invokePromise = invoke(command, args) as Promise<T>;
    return await Promise.race([invokePromise, timeoutPromise]);
  } catch (caught: unknown) {
    const error = caught as CaughtError;
    const isTimeoutError =
      error?.message === 'TIMEOUT' ||
      error?.message?.includes('Connection refused') ||
      error?.message?.includes('WebSocket') ||
      error?.message?.includes('timeout');

    // Never retry long-running, non-idempotent transcription commands -
    // retrying just piles up concurrent transcriptions and loses model cache
    const NO_RETRY_COMMANDS = [
      'transcribe_audio',
      'stop_audio_recording_and_transcribe',
      'transcribe_open_mic_chunk',
    ];

    // Transcription commands: no retry, no auto-restart - just surface the timeout
    if (isTimeoutError && NO_RETRY_COMMANDS.includes(command)) {
      console.error(`❌ "${command}" timed out after ${TRANSCRIPTION_TIMEOUT_MS / 1000}s`);
      throw new Error(
        translate('errors.transcriptionTimeout', {
          seconds: String(TRANSCRIPTION_TIMEOUT_MS / 1000),
        }),
        { cause: caught }
      );
    }

    // Auto-retry for timeout/connection errors on other commands
    if (isTimeoutError && retryCount < MAX_RETRIES) {
      const delay = RETRY_DELAYS[retryCount] || 4000;
      console.log(`⏳ Retry ${retryCount + 1}/${MAX_RETRIES} for "${command}" in ${delay}ms...`);
      await new Promise(resolve => setTimeout(resolve, delay));
      return safeInvoke<T>(command, args, retryCount + 1);
    }

    // Max retries exceeded - trigger auto-restart
    if (isTimeoutError) {
      console.error(`❌ Max retries exceeded for "${command}". Triggering auto-restart...`);
      // Give user a moment to see what's happening
      setTimeout(() => triggerAppRestart(), 1500);
      throw new Error(translate('errors.connectionLost'), { cause: caught });
    }

    // Only treat specific initialization errors as server startup issues
    if (
      error?.message?.includes('command not found') ||
      error?.message?.includes('not registered') ||
      error?.message?.includes('failed to initialize')
    ) {
      throw new Error(translate('errors.serverInitializing'), { cause: caught });
    }
    // For other errors (like microphone issues), pass them through as-is
    throw caught;
  }
}

// Configuration constants
const RECORDING_TIME_UPDATE_INTERVAL_MS = 1000;
const MIN_RECORDING_TIME_SECONDS = 5; // Minimum 5 seconds
// Maximum 180 seconds (3 minutes). A hard memory ceiling: the whole clip goes to the
// model in one call, and a single 300s clip pushed a real install to ~4.5 GB.
const MAX_RECORDING_TIME_SECONDS = 180;
const DEFAULT_RECORDING_TIME_SECONDS = defaultSettings.max_recording_time ?? 30;

// Allow configuration but clamp between min and max.
//
// The user preference is read from the Tauri settings store - the same store
// the settings UI writes to. It used to be read out of a `echophrase-settings`
// localStorage key under a camelCase name that nothing ever wrote, so the
// duration picker had no effect and every recording auto-stopped at 30s.
function getMaxRecordingTime(): number {
  const userSetting = get(tauriSettings)?.max_recording_time ?? DEFAULT_RECORDING_TIME_SECONDS;

  // Clamp the user setting to the hard ceiling (also catches values persisted
  // by older builds that allowed up to 300s).
  return Math.max(MIN_RECORDING_TIME_SECONDS, Math.min(MAX_RECORDING_TIME_SECONDS, userSetting));
}

/**
 * Recording functionality - extracted from +page.svelte
 */
export function useRecording(getModelExists?: () => boolean, showDownloadDialog?: () => void) {
  // Reactive state - exactly like the original variables
  const isRecording = writable(false);
  const isProcessing = writable(false);
  const recordingTime = writable(0);
  const errorMessage = writable('');
  const serverStatus = writable<'connecting' | 'ready' | 'error'>('connecting');

  // Open mic state
  const isOpenMicActive = writable(false);
  const isOpenMicListening = writable(false);
  const isOpenMicSilenceCounting = writable(false);
  const openMicChunkCount = writable(0);

  // Open mic internal state
  let openMicUnlisteners: UnlistenFn[] = [];
  let openMicSessionTimer: NodeJS.Timeout | null = null;
  let pendingChunkRequest = false;
  const OPEN_MIC_MAX_SESSION_MS = 30 * 60 * 1000; // 30 minutes safety limit

  // Session generation counter. Incremented once per startOpenMicSession()
  // call and captured into a local `sessionId`. Cleanup and event handlers
  // compare their captured id against this current value and no-op on a
  // mismatch, so a losing concurrent start (Rust returns "Already recording")
  // can never tear down the winning session's listeners/state - only the
  // most recent start "owns" openMicUnlisteners/openMicSessionTimer/recordingInterval.
  let openMicSessionGeneration = 0;

  // Internal state
  let recordingInterval: NodeJS.Timeout | null;
  let serverCheckInterval: number;

  // Synchronous re-entrancy guard for toggleRecording(). isProcessing/isRecording
  // stores only flip AFTER the first await inside startRecording/startOpenMicSession,
  // so a near-simultaneous duplicate hotkey dispatch (see useHotkeys.ts) could
  // race in through that gap and start two sessions. This flag closes the gap:
  // set synchronously on entry, cleared in a finally once the dispatch itself
  // (not its async body) has completed.
  let toggleInFlight = false;

  // A start (push-to-talk or open mic) has been dispatched but the recorder
  // isn't up yet (isRecording flips only after the mic opens). `startSeq` counts
  // dispatched starts so a hotkey handler can tell "my start" from a later one.
  let startPending = false;
  let startSeq = 0;
  /** Longest a dispatched start may hold the pending slot (mic open normally takes < 2s). */
  const START_PENDING_MAX_MS = 10_000;

  // Initialize Tauri APIs on first use
  let apisInitialized = false;
  async function ensureAPIsInitialized() {
    if (apisInitialized) return apisInitialized;

    // 10s timeout guard. If the Rust side is blocked (e.g. tokio spawn_blocking
    // pool saturated by a slow model preload), the init invoke can hang forever.
    // Before this guard, that produced a silent dead-end right after the
    // "🎙️ Toggle recording - can transcribe check: true" log.
    const INIT_TIMEOUT_MS = 10_000;
    try {
      apisInitialized = await Promise.race([
        initializeTauriAPIs(),
        new Promise<boolean>((_, reject) =>
          setTimeout(() => reject(new Error('INIT_TAURI_APIS_TIMEOUT')), INIT_TIMEOUT_MS)
        ),
      ]);
    } catch (e) {
      const isTimeout = e instanceof Error && e.message === 'INIT_TAURI_APIS_TIMEOUT';
      if (isTimeout) {
        console.error(
          `⏱️ initializeTauriAPIs() timed out after ${INIT_TIMEOUT_MS}ms. ` +
            `The Rust backend is not responding - most likely still loading the model. ` +
            `Check for "[ONNX] Preloading..." in the terminal log.`
        );
        errorMessage.set(translate('errors.connectionLost'));
        setTimeout(() => errorMessage.set(''), 5000);
      } else {
        console.error('❌ initializeTauriAPIs() threw:', e);
      }
      return false;
    }
    return apisInitialized;
  }

  // Check server status every 5 seconds
  function startServerStatusCheck() {
    if (typeof window !== 'undefined' && window.setInterval) {
      serverCheckInterval = window.setInterval(() => {
        if (isTauriAvailable()) {
          serverStatus.set('ready');
          clearInterval(serverCheckInterval);
        }
      }, 5000);
    }
  }

  // Initialize based on environment
  if (typeof window !== 'undefined') {
    // Browser environment
    if (!isTauriAvailable()) {
      // Web mode - no Tauri available
      serverStatus.set('error');
    } else {
      // Desktop mode - initialize APIs
      ensureAPIsInitialized()
        .then(success => {
          if (success) {
            serverStatus.set('ready');
            // Also try a quick test to see if commands actually work
            safeInvoke('check_selected_model_exists')
              .then(() => {
                serverStatus.set('ready');
              })
              .catch(error => {
                console.warn('⚠️ Model check failed during init:', error);
                // If commands don't work yet, start polling
                if (get(serverStatus) !== 'ready') {
                  startServerStatusCheck();
                }
              });
          } else {
            startServerStatusCheck();
          }
        })
        .catch(error => {
          console.error('❌ Failed to initialize APIs:', error);
          serverStatus.set('error');
        });
    }
  } else {
    // Test environment or SSR
    serverStatus.set('error');
  }

  async function startRecording() {
    console.log('🎙️ Starting recording (model existence already verified)...');

    // Ensure APIs are initialized
    const apisReady = await ensureAPIsInitialized();
    if (!apisReady) {
      errorMessage.set(translate('errors.webModeRecording'));
      setTimeout(() => errorMessage.set(''), 5000);
      return;
    }

    try {
      // One in-memory Rust recorder on every platform (echo-audio/cpal).
      // The command owns the tray status; no JS-side sync needed.
      await safeInvoke('start_audio_recording');

      recordingTime.set(0);
      isRecording.set(true);
      beginRun();

      // Start recording timer that updates every second
      // This timer also enforces the max recording duration
      if (recordingInterval) clearInterval(recordingInterval);
      recordingInterval = setInterval(() => {
        recordingTime.update(time => {
          const newTime = time + 1;

          // Check if we've hit the user-configured maximum recording time
          // This prevents accidental long recordings that could use excessive memory
          const maxTime = getMaxRecordingTime();
          if (newTime >= maxTime) {
            console.log(`⏱️ Max recording time reached (${maxTime}s), auto-stopping...`);
            // Automatically stop and process the recording
            stopRecording();
            return maxTime;
          }

          return newTime;
        });
      }, RECORDING_TIME_UPDATE_INTERVAL_MS);

      console.log('✅ Recording started successfully');
    } catch (caught: unknown) {
      const error = caught as CaughtError;
      console.error('❌ Error starting recording:', error);
      isRecording.set(false);

      // Check for connection/restart errors
      if (error?.message?.includes('Connection lost') || error?.message?.includes('Restarting')) {
        errorMessage.set(translate('errors.connectionLost'));
        // Don't auto-clear - app will restart
      } else if (
        error?.toString?.().includes('no longer available') ||
        error?.toString?.().includes('unplugged')
      ) {
        // Check if we're in WSL
        if (typeof window !== 'undefined' && window.location.hostname === 'localhost') {
          errorMessage.set(translate('errors.wslMicLimitation'));
        } else {
          errorMessage.set(translate('errors.micNotAvailable'));
        }
        setTimeout(() => errorMessage.set(''), 5000);
      } else if (error?.toString?.().includes('permission')) {
        errorMessage.set(translate('errors.micPermissionDenied'));
        setTimeout(() => errorMessage.set(''), 5000);
      } else if (error?.toString?.().includes('ALSA')) {
        errorMessage.set(translate('errors.audioNotConfigured'));
        setTimeout(() => errorMessage.set(''), 5000);
      } else if (
        error?.toString?.().includes('parec') ||
        error?.toString?.().toLowerCase().includes('pulseaudio')
      ) {
        // WSL/Linux PulseAudio capture failed (parec missing or source dead).
        errorMessage.set(translate('errors.wslCaptureFailed'));
        setTimeout(() => errorMessage.set(''), 7000);
      } else if (
        error?.toString?.().includes('no_input_devices') ||
        error?.toString?.().includes('backend-specific') ||
        error?.toString?.().includes('coreaudio')
      ) {
        errorMessage.set(translate('errors.noMicrophoneDetected'));
        setTimeout(() => errorMessage.set(''), 7000);
      } else if (
        error?.toString?.().includes('Already recording') ||
        error?.toString?.().includes('already in progress')
      ) {
        // Stuck recording state detected - auto-reset and retry
        console.log('🔄 Stuck recording state detected, resetting...');
        try {
          await safeInvoke('reset_recording_state');
          console.log('✅ Recording state reset, retrying...');
          // Retry recording after reset
          setTimeout(() => startRecording(), 500);
        } catch (resetError) {
          console.error('❌ Failed to reset recording state:', resetError);
          errorMessage.set(translate('errors.recordingStuck'));
          setTimeout(() => errorMessage.set(''), 5000);
        }
      } else {
        errorMessage.set(
          translate('errors.recordingFailed', { error: String(error?.message || error) })
        );
        setTimeout(() => errorMessage.set(''), 5000);
      }
    }
  }

  async function stopRecording() {
    // CRITICAL: Clear interval FIRST to prevent re-entry loops
    if (recordingInterval) {
      clearInterval(recordingInterval);
      recordingInterval = null;
    }

    try {
      console.log('🛑 Attempting to stop recording...');

      // Ensure APIs are initialized
      const apisReady = await ensureAPIsInitialized();
      if (!apisReady) {
        isRecording.set(false);
        errorMessage.set(translate('errors.webModeDisabled'));
        setTimeout(() => errorMessage.set(''), 5000);
        return;
      }

      // Immediately set processing state
      isRecording.set(false);
      isProcessing.set(true);

      let transcribedText: string;
      let originalTextBeforeTransform: string | null = null; // Track original for transform toast
      let transcriptionSeconds: number;
      let transformSeconds: number | null = null;

      {
        console.log('🛑 Stopping recorder and transcribing directly from memory (no file I/O)');
        const transcriptionStart = performance.now();
        // Combined stop + transcribe for direct buffer processing
        const rawText = await safeInvoke<string>('stop_audio_recording_and_transcribe');
        transcriptionSeconds = (performance.now() - transcriptionStart) / 1000;
        devLog(`🎤 Raw transcription (${transcriptionSeconds.toFixed(2)}s): ${rawText}`);

        // Apply transform if enabled (WSL path). Skip entirely for the
        // silence sentinel - there's nothing to grammar-correct, and running
        // the transform model on it is a wasted inference.
        transcribedText = rawText;
        try {
          const settings = get(tauriSettings);
          if (settings?.transform_enabled && !isSilenceResult(rawText)) {
            const modelName = settings.transform_model || 't5-mini-grammar';
            console.log(`🔄 Applying transform [${modelName}]...`);
            const transformStart = performance.now();
            const transformed = await safeInvoke<string>('transform_text', {
              text: rawText,
              rules: settings.transform_rules || null,
            });
            transformSeconds = (performance.now() - transformStart) / 1000;
            if (transformed !== rawText) {
              devLog(`✨ Transformed (${transformSeconds.toFixed(2)}s): ${transformed}`);
              originalTextBeforeTransform = rawText; // Save original for transform toast
              transcribedText = transformed;
            } else {
              console.log(`✅ Transform complete (${transformSeconds.toFixed(2)}s): (no change)`);
            }
          }
        } catch (transformError) {
          console.warn('⚠️ Transform failed, using original transcription:', transformError);
        }
      }

      const totalSeconds = transcriptionSeconds + (transformSeconds ?? 0);
      console.log(`✅ Transcription completed in ${totalSeconds.toFixed(1)}s`);
      devLog('   Transcript:', transcribedText);

      // Emit transcription-segment for preview display (covers ONNX backends
      // that don't emit streaming events from Rust)
      if (transcribedText && transcribedText.trim().length > 0) {
        // If transform was applied, show before→after animation
        if (originalTextBeforeTransform) {
          // First show original text
          window.dispatchEvent(
            new CustomEvent('transcription-preview', {
              detail: {
                text: originalTextBeforeTransform,
                is_final: false,
                isOriginalBeforeTransform: true,
              },
            })
          );

          // After a short delay, show transformed text with animation trigger
          setTimeout(() => {
            window.dispatchEvent(
              new CustomEvent('transcription-preview', {
                detail: {
                  text: transcribedText,
                  is_final: true,
                  isTransformed: true,
                  originalText: originalTextBeforeTransform,
                },
              })
            );
          }, 1500); // Show original for 1.5s before transitioning
        } else {
          // No transform - just show the text
          window.dispatchEvent(
            new CustomEvent('transcription-preview', {
              detail: {
                text: transcribedText,
                is_final: true,
                isSilence: isSilenceResult(transcribedText),
              },
            })
          );
        }
      }

      // Show speed toast if enabled
      const currentSettings = get(tauriSettings);
      if (currentSettings?.show_transcription_speed === true) {
        showSpeedToast(transcriptionSeconds, transformSeconds);
      }

      isProcessing.set(false);

      // Create a proper TranscriptionResult object
      const transcriptionResult = {
        id: Date.now().toString(),
        text: transcribedText,
        timestamp: new Date(),
        language: 'en',
        model: 'faster-whisper-large-v3-turbo',
        duration: get(recordingTime),
      };

      // Update the transcription store with the result
      const { transcription } = await import('$lib/stores/transcription');
      transcription.completeTranscription(transcriptionResult);

      // Handle output (clipboard copy, paste at cursor, save to file)
      // Supports multiple outputs - all selected modes are executed
      if (transcribedText && transcribedText !== 'No speech detected') {
        try {
          const settings = get(tauriSettings);

          // Apply filler word removal if enabled (default: true)
          let cleanedText = transcribedText;
          if (settings?.remove_filler_words !== false) {
            cleanedText = removeFillerWords(transcribedText);
            if (cleanedText !== transcribedText) {
              console.log('🧹 Filler words removed from output');
            }
          }

          // Apply number-to-digit conversion if enabled
          if (settings?.use_numeric_digits === true) {
            const beforeConversion = cleanedText;
            cleanedText = convertNumberWordsToDigits(cleanedText);
            if (cleanedText !== beforeConversion) {
              console.log('🔢 Number words converted to digits');
            }
          }

          // Append trailing space when the user paused mid-sentence (no
          // sentence-ending punctuation) so the next dictation flows naturally.
          if (settings?.append_space_incomplete !== false) {
            cleanedText = appendTrailingSpaceIfIncomplete(cleanedText);
          }

          // Update transcriptionResult with cleaned text
          const finalResult = {
            ...transcriptionResult,
            text: cleanedText,
          };

          // Format output based on output_format setting (text, json, srt)
          const formatter = useTranscriptFormatter();
          const outputFormat = settings?.output_format || 'text';
          const formattedOutput = formatter.formatOutput(finalResult, outputFormat, {
            customTemplate: settings?.json_metadata_template || undefined,
          });

          if (outputFormat !== 'text') {
            console.log(`📄 Formatted output as ${outputFormat}`);
          }

          const succeededActions = await deliverOutput(formattedOutput, settings);

          // Show transform toast if transform was applied
          if (originalTextBeforeTransform) {
            addOutputToast('transformed', {
              text: cleanedText,
              originalText: originalTextBeforeTransform,
            });
          }

          // Publish the aggregated last-run summary to drive the left-side chip stack.
          const lastRunSpeed: LastRunSpeed | null =
            currentSettings?.show_transcription_speed === true
              ? {
                  transcriptionSeconds,
                  transformSeconds,
                  totalSeconds: transcriptionSeconds + (transformSeconds ?? 0),
                }
              : null;
          publishRun({
            raw: originalTextBeforeTransform ?? transcribedText,
            transformed: originalTextBeforeTransform ? transcribedText : null,
            output: formattedOutput,
            actions: succeededActions,
            speed: lastRunSpeed,
            at: Date.now(),
          });
        } catch (outputError) {
          console.warn('⚠️ Failed to handle output:', outputError);
          // Don't fail the transcription if output handling fails
        }
      }
    } catch (error) {
      console.error('❌ Failed to stop recording:', error);

      isRecording.set(false);
      isProcessing.set(false);
      // Rust-side StatusGuard restores the tray on every exit path; a
      // best-effort reset covers failures before the command even ran.
      void safeInvoke('reset_recording_state').catch(() => {});

      if (recordingInterval) {
        clearInterval(recordingInterval);
      }

      const errorMsg = error instanceof Error ? error.message : String(error);
      errorMessage.set(errorMsg);
      console.log('⚠️ Setting error message:', errorMsg);
      setTimeout(() => errorMessage.set(''), 5000);
    }
  }

  /**
   * Cancel the current recording without transcribing or processing the audio.
   * Handles both standard push-to-talk and open mic modes.
   * Stops the mic, discards the audio buffer, and returns to idle state.
   * Called when the user presses ESC during recording.
   */
  async function cancelRecording() {
    console.log('🚫 Cancelling recording (no transcription)...');

    // If open mic is active, cancel that session instead
    if (get(isOpenMicActive)) {
      console.log('🚫 Cancelling open mic session...');
      cleanupOpenMicState();
      try {
        const apisReady = await ensureAPIsInitialized();
        if (apisReady) {
          await safeInvoke('reset_recording_state');
        }
      } catch (error) {
        console.warn('⚠️ Open mic cancel cleanup warning:', error);
      }
      console.log('✅ Open mic session cancelled - no transcription');
      return;
    }

    // CRITICAL: Clear interval FIRST to prevent auto-stop re-entry
    if (recordingInterval) {
      clearInterval(recordingInterval);
      recordingInterval = null;
    }

    isRecording.set(false);
    isProcessing.set(false);
    recordingTime.set(0);
    errorMessage.set('');

    try {
      const apisReady = await ensureAPIsInitialized();
      if (!apisReady) {
        return;
      }

      // reset_recording_state stops the capture stream and discards the
      // in-memory buffer - nothing on disk to clean up.
      await safeInvoke('reset_recording_state');
      console.log('✅ Recording cancelled - buffer discarded');
    } catch (error) {
      // Non-fatal: mic state may already be clean
      console.warn('⚠️ Cancel recording cleanup warning:', error);
    }
  }

  // ========================================================================
  // Open Mic Session Management
  // ========================================================================

  async function startOpenMicSession() {
    const apisReady = await ensureAPIsInitialized();
    if (!apisReady) {
      errorMessage.set(translate('errors.webModeRecording'));
      setTimeout(() => errorMessage.set(''), 5000);
      return;
    }

    // Claim this call's own generation. Any cleanup/listener callback whose
    // captured id no longer matches openMicSessionGeneration belongs to a
    // superseded (losing) session and must no-op.
    const sessionId = ++openMicSessionGeneration;

    try {
      const settings = get(tauriSettings);
      const threshold = (settings?.voice_threshold ?? 50) / 100;
      const silenceTimeout = (settings?.silence_timeout ?? 5) * 1000; // Convert to ms

      // Setup event listeners
      const { listen } = await import('@tauri-apps/api/event');

      const unlistenChunk = await listen('open-mic-chunk-ready', () => {
        if (sessionId !== openMicSessionGeneration) return; // stale listener from a superseded session
        handleOpenMicChunk();
      });
      const unlistenSpeech = await listen('open-mic-speech-started', () => {
        if (sessionId !== openMicSessionGeneration) return;
        isOpenMicListening.set(false);
        isOpenMicSilenceCounting.set(false);
      });

      const unlistenSilence = await listen('open-mic-silence-started', () => {
        if (sessionId !== openMicSessionGeneration) return;
        isOpenMicSilenceCounting.set(true);
      });

      // Fires when the backend discards a false VAD trigger (speech too short
      // to be real). Snap the UI straight back to "Listening" instead of
      // cycling through the silence countdown.
      const unlistenResetToListening = await listen('open-mic-reset-to-listening', () => {
        if (sessionId !== openMicSessionGeneration) return;
        isOpenMicListening.set(true);
        isOpenMicSilenceCounting.set(false);
      });

      const unlisteners = [
        unlistenChunk,
        unlistenSpeech,
        unlistenSilence,
        unlistenResetToListening,
      ];

      // If a newer session already started while we were awaiting the listen()
      // calls above, this call lost the race - tear down only OUR listeners
      // and stop, without touching the winning session's state.
      if (sessionId !== openMicSessionGeneration) {
        for (const unlisten of unlisteners) unlisten();
        return;
      }

      // Start VAD-enhanced recording
      await safeInvoke('start_audio_recording_with_vad', {
        threshold,
        silenceTimeoutMs: silenceTimeout,
      });

      // Re-check after the await above too - Rust may reject a losing
      // concurrent call with "Already recording", which lands in the catch
      // block below, but a losing call could in principle also resolve
      // after a newer session has already taken over.
      if (sessionId !== openMicSessionGeneration) {
        for (const unlisten of unlisteners) unlisten();
        return;
      }

      openMicUnlisteners = unlisteners;

      isOpenMicActive.set(true);
      isOpenMicListening.set(true);
      openMicChunkCount.set(0);
      recordingTime.set(0);
      isRecording.set(true);

      // Recording time counter (clear first so a duplicate start can't leak one)
      if (recordingInterval) clearInterval(recordingInterval);
      recordingInterval = setInterval(() => {
        recordingTime.update(t => t + 1);
      }, RECORDING_TIME_UPDATE_INTERVAL_MS);

      // Safety timeout: auto-stop after 30 minutes
      openMicSessionTimer = setTimeout(() => {
        if (sessionId !== openMicSessionGeneration) return;
        console.log('Open mic session safety timeout reached (30 min), stopping...');
        stopOpenMicSession();
      }, OPEN_MIC_MAX_SESSION_MS);

      console.log(
        'Open mic session started (threshold:',
        threshold,
        ', timeout:',
        silenceTimeout,
        'ms)'
      );
    } catch (caught: unknown) {
      const error = caught as CaughtError;
      if (sessionId !== openMicSessionGeneration) {
        // Losing concurrent start (e.g. Rust returned "Already recording").
        // The winning session already owns openMicUnlisteners/recordingInterval -
        // do NOT call cleanupOpenMicState() here, it would tear down the winner.
        console.log('Open mic: ignoring failure from a superseded concurrent start');
        return;
      }
      console.error('Failed to start open mic session:', error);
      cleanupOpenMicState(sessionId);
      errorMessage.set(
        translate('errors.recordingFailed', { error: String(error?.message || error) })
      );
      setTimeout(() => errorMessage.set(''), 5000);
    }
  }

  async function stopOpenMicSession() {
    // Stop recording timer
    if (recordingInterval) {
      clearInterval(recordingInterval);
      recordingInterval = null;
    }

    // Stop safety timer
    if (openMicSessionTimer) {
      clearTimeout(openMicSessionTimer);
      openMicSessionTimer = null;
    }

    try {
      // Stop the VAD recorder - this also stops PulseAudio
      isRecording.set(false);
      isProcessing.set(true);

      // Stop recording and get any remaining audio
      const remainingText = await safeInvoke<string>('stop_audio_recording_and_transcribe');

      isProcessing.set(false);

      // Handle remaining audio if it has content
      if (
        remainingText &&
        remainingText.trim().length > 0 &&
        remainingText !== 'No speech detected'
      ) {
        await handleOpenMicOutput(remainingText);
      }
    } catch (error) {
      console.error('Error stopping open mic session:', error);
      isProcessing.set(false);
    } finally {
      cleanupOpenMicState();
    }
  }

  /**
   * Tear down the active open mic session's listeners/timers/stores.
   *
   * @param sessionId - When provided (called from within startOpenMicSession's
   * own error path), only proceeds if it still matches the current generation -
   * guards against a losing concurrent start clobbering a session that has
   * since been superseded by a newer one. Callers acting on user intent
   * (cancelRecording, stopOpenMicSession, component unmount) omit it and
   * always clean up the current active session.
   */
  function cleanupOpenMicState(sessionId?: number) {
    if (sessionId !== undefined && sessionId !== openMicSessionGeneration) return;

    isOpenMicActive.set(false);
    isOpenMicListening.set(false);
    isRecording.set(false);
    isProcessing.set(false);
    pendingChunkRequest = false;

    for (const unlisten of openMicUnlisteners) {
      unlisten();
    }
    openMicUnlisteners = [];

    if (openMicSessionTimer) {
      clearTimeout(openMicSessionTimer);
      openMicSessionTimer = null;
    }
    if (recordingInterval) {
      clearInterval(recordingInterval);
      recordingInterval = null;
    }
  }

  async function handleOpenMicChunk() {
    // If already processing a chunk, remember it for later (Rust queues the buffer)
    if (get(isProcessing)) {
      pendingChunkRequest = true;
      console.log('Open mic: chunk arrived while processing, queued for retry');
      return;
    }

    await processOpenMicChunk();
  }

  async function processOpenMicChunk() {
    let timedOut = false;
    try {
      beginRun();
      isProcessing.set(true);
      isOpenMicListening.set(false);
      isOpenMicSilenceCounting.set(false);

      const transcriptionStart = performance.now();
      // Watchdog: an open mic chunk is a few seconds of speech at most, so a
      // request that hasn't resolved within OPEN_MIC_CHUNK_TIMEOUT_MS is stuck
      // (not just slow). Without this, a single wedged invoke would leave
      // isProcessing stuck true forever and the pending_queue backlog would
      // only grow, since nothing ever calls back into handleOpenMicChunk().
      const result = await promiseWithTimeout(
        safeInvoke<{ text: string; audio_duration_ms: number }>('transcribe_open_mic_chunk'),
        OPEN_MIC_CHUNK_TIMEOUT_MS
      );
      const transcriptionSeconds = (performance.now() - transcriptionStart) / 1000;
      const text = result?.text ?? '';
      const audioDurationMs = result?.audio_duration_ms ?? 0;

      console.log(`Open mic chunk transcription duration: ${transcriptionSeconds.toFixed(2)}s`);

      if (text && text.trim().length > 0 && text !== 'No speech detected') {
        openMicChunkCount.update(n => n + 1);
        console.log(
          `Open mic chunk transcribed (inference ${transcriptionSeconds.toFixed(2)}s, audio ${(audioDurationMs / 1000).toFixed(2)}s): ${text.substring(0, 50)}...`
        );

        await handleOpenMicOutput(text, transcriptionSeconds);

        // Show speed toast if enabled
        const currentSettings = get(tauriSettings);
        if (currentSettings?.show_transcription_speed === true) {
          showSpeedToast(transcriptionSeconds);
        }
      }
    } catch (error) {
      if (error instanceof TimeoutError) {
        timedOut = true;
        console.error(
          `Open mic chunk transcription timed out after ${OPEN_MIC_CHUNK_TIMEOUT_MS / 1000}s - discarding any queued chunk`
        );
      } else {
        console.error('Open mic chunk transcription failed:', error);
      }
    } finally {
      // Reset the in-flight flag on every path (success, error, timeout) so
      // the open mic loop can never get permanently wedged on one bad chunk.
      isProcessing.set(false);
      isOpenMicListening.set(true);
      isOpenMicSilenceCounting.set(false);

      if (timedOut) {
        // A stuck request means we can't trust the Rust-side pending_queue
        // state either - drop any queued chunk instead of immediately firing
        // another request at a backend that may still be wedged on the last one.
        pendingChunkRequest = false;
      } else if (pendingChunkRequest && get(isOpenMicActive)) {
        // Drain any queued chunk from the Rust pending_queue
        pendingChunkRequest = false;
        setTimeout(() => handleOpenMicChunk(), 50);
      }
    }
  }

  /** Handle output for open mic - runs the user's output modes for each VAD chunk */
  async function handleOpenMicOutput(text: string, transcriptionSeconds: number = 0) {
    try {
      const settings = get(tauriSettings);

      // Apply filler word removal
      let cleanedText = text;
      if (settings?.remove_filler_words !== false) {
        cleanedText = removeFillerWords(text);
      }

      // Apply number-to-digit conversion if enabled
      if (settings?.use_numeric_digits === true) {
        cleanedText = convertNumberWordsToDigits(cleanedText);
      }

      // Append trailing space for incomplete sentences (mid-sentence pause)
      if (settings?.append_space_incomplete !== false) {
        cleanedText = appendTrailingSpaceIfIncomplete(cleanedText);
      }

      // Format output
      const formatter = useTranscriptFormatter();
      const outputFormat = settings?.output_format || 'text';
      const formattedOutput = formatter.formatOutput(
        {
          id: Date.now().toString(),
          text: cleanedText,
          timestamp: new Date(),
          language: 'en',
          model: 'whisper',
          duration: 0,
        },
        outputFormat,
        { customTemplate: settings?.json_metadata_template || undefined }
      );

      // Same output modes as push-to-talk (minus save-as-file without an
      // auto-save directory, see planOutputModes). This used to force
      // clipboard-only, which silently ignored a user's "Paste at cursor" setting.
      const actions = await deliverOutput(formattedOutput, settings, true);

      // Publish aggregated last-run summary for the left chip stack (each chunk = a micro-run).
      const lastRunSpeed: LastRunSpeed | null =
        settings?.show_transcription_speed === true && transcriptionSeconds > 0
          ? {
              transcriptionSeconds,
              transformSeconds: null,
              totalSeconds: transcriptionSeconds,
            }
          : null;
      publishRun({
        raw: cleanedText,
        transformed: null,
        output: formattedOutput,
        actions,
        speed: lastRunSpeed,
        at: Date.now(),
      });

      // Update transcription store
      const { transcription } = await import('$lib/stores/transcription');
      transcription.completeTranscription({
        id: Date.now().toString(),
        text: cleanedText,
        timestamp: new Date(),
        language: 'en',
        model: 'whisper',
        duration: 0,
      });

      // Emit preview event
      window.dispatchEvent(
        new CustomEvent('transcription-preview', {
          detail: { text: cleanedText, is_final: true },
        })
      );
    } catch (error) {
      console.warn('Failed to handle open mic output:', error);
    }
  }

  async function toggleRecording() {
    // Defensive re-entrancy guard: a near-simultaneous duplicate call (e.g. a
    // single hotkey press somehow dispatched twice, see useHotkeys.ts) is a
    // no-op. This closes the gap the isProcessing check below can't cover:
    // isProcessing/isRecording only flip AFTER the first await inside
    // startRecording/startOpenMicSession, so a duplicate call arriving before
    // that point would otherwise sail right through.
    if (toggleInFlight) {
      console.log('⚠️ toggleRecording re-entrant call ignored (already in flight)');
      return;
    }
    // A start is still opening the mic: a second toggle must not start another
    // session (and can't stop one that isn't running yet).
    if (startPending) {
      console.log('⚠️ toggleRecording ignored (a start is already pending)');
      return;
    }
    toggleInFlight = true;
    // Claim the pending-start slot synchronously, before the first await, so a
    // press arriving during that await sees it. Released below if no start
    // was actually dispatched, or by the start itself when it settles.
    const mayStart = !get(isRecording) && !get(isOpenMicActive) && !get(isProcessing);
    if (mayStart) {
      startPending = true;
      startSeq++;
    }
    let dispatched = false;
    try {
      dispatched = await toggleRecordingImpl();
    } finally {
      toggleInFlight = false;
      if (mayStart && !dispatched) startPending = false;
    }
    // Safety valve: if the mic open ever hangs, don't block every toggle until
    // restart. Only releases the slot if no newer start has claimed it.
    if (mayStart && dispatched) {
      const mySeq = startSeq;
      setTimeout(() => {
        if (startPending && startSeq === mySeq) startPending = false;
      }, START_PENDING_MAX_MS);
    }
  }

  /** Returns true when a start was dispatched (it then owns clearing `startPending`). */
  async function toggleRecordingImpl(): Promise<boolean> {
    // Block recording on unsupported Macs (Intel x86_64)
    if (platformCache.isUnsupportedMac) {
      errorMessage.set(translate('errors.intelMacUnsupported'));
      setTimeout(() => errorMessage.set(''), 7000);
      return false;
    }

    // Ensure APIs are initialized first
    const apisReady = await ensureAPIsInitialized();
    if (!apisReady) {
      errorMessage.set(translate('errors.webModeRecording'));
      setTimeout(() => errorMessage.set(''), 5000);
      return false;
    }

    // Prevent multiple simultaneous attempts
    if (get(isProcessing)) {
      console.log('⚠️ Recording action blocked - currently processing');
      return false;
    }

    // Check if open mic session is active - toggle stops it
    if (get(isOpenMicActive)) {
      stopOpenMicSession();
      return false;
    }

    if (get(isRecording)) {
      stopRecording();
      return false;
    } else {
      // Check microphone availability first
      // Check if model exists BEFORE starting recording
      const modelExists = getModelExists?.();
      console.log('🎙️ Toggle recording - model exists check:', modelExists);
      if (!modelExists) {
        console.log('❌ Whisper model not found - showing download dialog');
        // Show download dialog instead of error message
        if (showDownloadDialog) {
          showDownloadDialog();
        } else {
          // Fallback to error message if no dialog callback provided
          errorMessage.set(translate('errors.downloadModelFirst'));
          setTimeout(() => errorMessage.set(''), 5000);
        }
        return false; // Exit without starting recording
      }

      // Model exists and microphone available
      // Branch based on recording mode
      const settings = get(tauriSettings);
      const recordingMode = settings?.recording_mode || 'push_to_talk';

      console.log(
        `🔀 Dispatching to ${recordingMode === 'open_mic' ? 'startOpenMicSession' : 'startRecording'} (mode=${recordingMode})`
      );

      // Catch fire-and-forget rejections so a throw inside startRecording/startOpenMicSession
      // doesn't silently dead-end (prior behaviour: no log past "can transcribe check").
      if (recordingMode === 'open_mic') {
        startOpenMicSession()
          .catch(e => console.error('❌ startOpenMicSession threw:', e))
          .finally(() => {
            startPending = false;
          });
      } else {
        startRecording()
          .catch(e => console.error('❌ startRecording threw:', e))
          .finally(() => {
            startPending = false;
          });
      }
      return true;
    }
  }

  // TODO: Re-enable as separate "transcription from file" feature - allows users to upload audio files for transcription
  // async function mockTranscription(audioPath: string): Promise<string> {
  //   // Simulate processing time
  //   await new Promise(resolve => setTimeout(resolve, 2000));
  //   return `Mock transcription of audio file: ${audioPath} recorded at ${new Date().toLocaleTimeString()}`;
  // }

  function formatTime(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  }

  function cleanup(): void {
    if (recordingInterval) {
      clearInterval(recordingInterval);
    }
    cleanupOpenMicState();
  }

  return {
    // State
    isRecording: { subscribe: isRecording.subscribe },
    isProcessing: { subscribe: isProcessing.subscribe },
    recordingTime: { subscribe: recordingTime.subscribe },
    errorMessage: { subscribe: errorMessage.subscribe },
    serverStatus: { subscribe: serverStatus.subscribe },
    // Open mic state
    isOpenMicActive: { subscribe: isOpenMicActive.subscribe },
    isOpenMicListening: { subscribe: isOpenMicListening.subscribe },
    isOpenMicSilenceCounting: { subscribe: isOpenMicSilenceCounting.subscribe },
    openMicChunkCount: { subscribe: openMicChunkCount.subscribe },

    // Actions
    startRecording,
    stopRecording,
    cancelRecording,
    toggleRecording,
    /** True while a dispatched start is still opening the mic (isRecording not yet true). */
    isStartPending: () => startPending,
    /** Increments on every dispatched start; lets a caller recognise its own start. */
    getStartSeq: () => startSeq,
    formatTime,
    cleanup,
  };
}
