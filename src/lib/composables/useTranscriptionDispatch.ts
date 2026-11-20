// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * useTranscriptionDispatch
 *
 * Handles transcription of a recorded audio file and routes the result
 * through the output pipeline (clipboard, paste, file save, toast).
 */

import { get } from 'svelte/store';
import { beginRun, publishRun, type LastRunAction, type LastRunSpeed } from '$lib/stores/lastRun';
import { tauriSettings } from '$lib/composables/useTauriSettings';
import {
  removeFillerWords,
  convertNumberWordsToDigits,
  appendTrailingSpaceIfIncomplete,
  useTranscriptFormatter,
} from '$lib/composables/useTranscriptFormatter';
import { useOutputHandler, type OutputMode } from '$lib/composables/useOutputHandler';
import { translate } from '$lib/locales';
import { recordingInvoke } from '$lib/utils/recordingInvoke';

import { addOutputToast } from '$lib/stores/outputToasts';

// -------------------------------------------------------------------------
// Types
// -------------------------------------------------------------------------

export interface TranscribeResult {
  text: string;
  transcriptionSeconds: number;
  transformSeconds: number | null;
  /** Original text before transform was applied (null when no transform). */
  originalText: string | null;
}

// -------------------------------------------------------------------------
// Transcription
// -------------------------------------------------------------------------

/**
 * Invoke the backend transcription command and optionally apply the transform
 * model if enabled in settings.
 */
export async function transcribeAudio(
  audioPath: string,
  audioDurationSec: number
): Promise<TranscribeResult> {
  try {
    const transcriptionStart = performance.now();
    console.log('Invoking transcribe_audio backend...');

    const rawResult = await recordingInvoke<string>('transcribe_audio', {
      filePath: audioPath,
      audio_duration_sec: audioDurationSec,
    });

    const transcriptionSeconds = (performance.now() - transcriptionStart) / 1000;
    console.log(`Raw transcription (${transcriptionSeconds.toFixed(2)}s): ${rawResult}`);

    let result = rawResult;
    let transformSeconds: number | null = null;
    let originalText: string | null = null;

    try {
      const settings = get(tauriSettings);
      if (settings?.transform_enabled) {
        const modelName = settings.transform_model || 't5-efficient-tiny-grammar';
        console.log(`Applying transform [${modelName}]...`);

        const transformStart = performance.now();
        const transformed = await recordingInvoke<string>('transform_text', {
          text: rawResult,
          rules: settings.transform_rules || null,
        });
        transformSeconds = (performance.now() - transformStart) / 1000;

        if (transformed !== rawResult) {
          console.log(`Transformed (${transformSeconds.toFixed(2)}s): ${transformed}`);
          originalText = rawResult;
          result = transformed;
        } else {
          console.log(`Transform complete (${transformSeconds.toFixed(2)}s): (no change)`);
        }
      }
    } catch (transformError) {
      console.warn('Transform failed, using original transcription:', transformError);
    }

    return { text: result, transcriptionSeconds, transformSeconds, originalText };
  } catch (error) {
    console.error('Transcription failed:', error);
    return {
      text: `${translate('errors.transcriptionFailed')}: ${error}`,
      transcriptionSeconds: 0,
      transformSeconds: null,
      originalText: null,
    };
  }
}

// -------------------------------------------------------------------------
// Output routing
// -------------------------------------------------------------------------

export interface OutputRouteParams {
  transcribedText: string;
  originalTextBeforeTransform: string | null;
  transcriptionSeconds: number;
  transformSeconds: number | null;
  recordingDurationSeconds: number;
}

/**
 * Route transcribed text through all configured output modes, apply text
 * cleanup, emit preview event, and publish a last-run summary.
 */
export async function routeTranscriptionOutput(params: OutputRouteParams): Promise<void> {
  const {
    transcribedText,
    originalTextBeforeTransform,
    transcriptionSeconds,
    transformSeconds,
    recordingDurationSeconds,
  } = params;

  if (!transcribedText || transcribedText === 'No speech detected') return;

  try {
    const settings = get(tauriSettings);

    let cleanedText = transcribedText;

    if (settings?.remove_filler_words !== false) {
      cleanedText = removeFillerWords(transcribedText);
      if (cleanedText !== transcribedText) {
        console.log('Filler words removed from output');
      }
    }

    if (settings?.use_numeric_digits === true) {
      const before = cleanedText;
      cleanedText = convertNumberWordsToDigits(cleanedText);
      if (cleanedText !== before) {
        console.log('Number words converted to digits');
      }
    }

    // Append a trailing space when the user paused mid-sentence (no sentence-ending
    // punctuation), so the next dictation flows naturally instead of gluing together.
    if (settings?.append_space_incomplete !== false) {
      cleanedText = appendTrailingSpaceIfIncomplete(cleanedText);
    }

    const transcriptionResult = {
      id: Date.now().toString(),
      text: cleanedText,
      timestamp: new Date(),
      language: 'en',
      model: 'faster-whisper-large-v3-turbo',
      duration: recordingDurationSeconds,
    };

    const formatter = useTranscriptFormatter();
    const outputFormat = settings?.output_format || 'text';
    const formattedOutput = formatter.formatOutput(transcriptionResult, outputFormat, {
      customTemplate: settings?.json_metadata_template || undefined,
    });

    if (outputFormat !== 'text') {
      console.log(`Formatted output as ${outputFormat}`);
    }

    let outputModes: string[] = [];
    if (Array.isArray(settings?.output_to) && settings.output_to.length > 0) {
      outputModes = settings.output_to;
    } else if (typeof settings?.output_to === 'string' && settings.output_to !== 'none') {
      outputModes = [settings.output_to];
    } else {
      console.warn('No output_to setting found, defaulting to copy_to_clipboard');
      outputModes = ['copy_to_clipboard'];
    }

    console.log('Output modes from settings:', JSON.stringify(outputModes));

    const succeededActions: LastRunAction[] = [];
    if (outputModes.length > 0) {
      const outputHandler = useOutputHandler();
      for (const mode of outputModes) {
        const result = await outputHandler.handleOutput(formattedOutput, mode as OutputMode);
        if (result.success) {
          console.log(`Output handled successfully (${mode})`);
          if (
            mode === 'copy_to_clipboard' ||
            mode === 'paste_at_cursor' ||
            mode === 'save_as_file'
          ) {
            succeededActions.push(mode);
          }
          if (settings?.show_output_confirmations !== false) {
            addOutputToast(mode, {
              text: formattedOutput,
              filePath: (result as Record<string, unknown>).savedPath as string | undefined,
            });
          }
        } else {
          console.warn(`Output handling failed for ${mode}: ${result.error}`);
        }
      }
    }

    if (originalTextBeforeTransform) {
      addOutputToast('transformed', {
        text: cleanedText,
        originalText: originalTextBeforeTransform,
      });
    }

    const currentSettings = get(tauriSettings);
    const lastRunSpeed: LastRunSpeed | null =
      currentSettings?.show_transcription_speed === true
        ? {
            transcriptionSeconds,
            transformSeconds,
            totalSeconds: transcriptionSeconds + (transformSeconds ?? 0),
          }
        : null;

    beginRun();
    publishRun({
      raw: originalTextBeforeTransform ?? transcribedText,
      transformed: originalTextBeforeTransform ? transcribedText : null,
      output: formattedOutput,
      actions: succeededActions,
      speed: lastRunSpeed,
      at: Date.now(),
    });
  } catch (outputError) {
    console.warn('Failed to handle output:', outputError);
  }
}

/** Emit a transcription-preview CustomEvent on window. */
export function emitTranscriptionPreview(
  text: string,
  options: {
    isFinal: boolean;
    isTransformed?: boolean;
    originalText?: string;
    isOriginalBeforeTransform?: boolean;
  }
): void {
  window.dispatchEvent(
    new CustomEvent('transcription-preview', {
      detail: {
        text,
        is_final: options.isFinal,
        isTransformed: options.isTransformed,
        originalText: options.originalText,
        isOriginalBeforeTransform: options.isOriginalBeforeTransform,
      },
    })
  );
}
