// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import type { TranscriptionResult } from '$lib/types/transcription';

/**
 * Default JSON metadata template with placeholder variables
 */
export const DEFAULT_JSON_TEMPLATE = `{
  "id": "{{id}}",
  "text": "{{text}}",
  "timestamp": "{{timestamp}}",
  "language": "{{language}}",
  "model": "{{model}}",
  "duration": {{duration}}
}`;

/**
 * Common filler words/sounds to remove from transcriptions
 * Case-insensitive patterns that match whole words or standalone occurrences
 */
const FILLER_PATTERNS = [
  // English filler sounds
  /\b(uh|uhh|uhhh|um|umm|ummm|er|err|errr|ah|ahh|ahhh)\b/gi,
  // Repeated "like" as filler (but not "I like X")
  /\b(like,?\s+like)\b/gi,
  // "You know" as filler
  /\b(you know,?\s*)/gi,
  // "I mean" at start of sentences
  /^(I mean,?\s*)/gim,
  // Stutters/false starts (word-word pattern)
  /\b(\w+)-\1\b/gi,
];

/**
 * Remove filler words and sounds from text
 * Cleans up "um", "uh", "like", etc. and normalizes whitespace
 */
export function removeFillerWords(text: string): string {
  let cleaned = text;

  for (const pattern of FILLER_PATTERNS) {
    cleaned = cleaned.replace(pattern, ' ');
  }

  // Clean up resulting double spaces and trim
  cleaned = cleaned
    .replace(/\s{2,}/g, ' ') // Multiple spaces to single
    .replace(/\s+([.,!?])/g, '$1') // Remove space before punctuation
    .replace(/,\s*,/g, ',') // Remove double commas
    .trim();

  return cleaned;
}

/**
 * The app's own name, which speech models often split ("Echo phrase") or
 * lowercase. Matches "echo phrase", "echo-phrase" and "echophrase" in any case;
 * the trailing `\b` leaves "echo phrases" and "echo phrasebook" alone.
 */
const BRAND_NAME_PATTERN = /\becho[\s-]*phrase\b/gi;

/** Always-on fix: spell the app's name "Echophrase". */
export function fixBrandName(text: string): string {
  return text.replace(BRAND_NAME_PATTERN, 'Echophrase');
}

/**
 * Spoken punctuation phrase -> symbol. Leading whitespace/punctuation the model
 * put before the phrase and trailing punctuation after it are eaten, so
 * "working? Question mark." becomes "working?". `\b` on both ends keeps
 * "question marks" and "questionable" untouched. The optional tail captures the
 * whitespace + first letter of the next word so it can be capitalized.
 */
const SPOKEN_PUNCTUATION_PATTERN =
  /[\s,.;:!?]*\b(question[\s-]+mark|exclamation[\s-]+(?:mark|point))\b[,.;:!?]*(?:(\s+)(\p{L})?)?/giu;

/**
 * Convert spoken "question mark" / "exclamation mark" (or "point") into a
 * literal "?" / "!", capitalizing the word that follows.
 */
export function convertSpokenPunctuation(text: string): string {
  return text.replace(
    SPOKEN_PUNCTUATION_PATTERN,
    (_match, phrase: string, space?: string, nextLetter?: string) => {
      const symbol = phrase.toLowerCase().startsWith('question') ? '?' : '!';
      return symbol + (space ?? '') + (nextLetter?.toUpperCase() ?? '');
    }
  );
}

/**
 * Append a trailing space when the text does not end with sentence-ending
 * punctuation (. ! ?). Lets the user pause mid-sentence and keep dictating
 * without the next chunk gluing onto the previous one.
 *
 * Only affects plain-text output; JSON/templated formats are handled by the
 * caller before formatting. Returns the text unchanged when it already ends in
 * terminal punctuation or is empty/whitespace-only.
 */
export function appendTrailingSpaceIfIncomplete(text: string): string {
  const trimmedEnd = text.replace(/\s+$/, '');
  if (trimmedEnd.length > 0 && !/[.!?]$/.test(trimmedEnd)) {
    return trimmedEnd + ' ';
  }
  return text;
}

/**
 * Number word to digit mapping
 * Covers 0-20, decades, and large numbers
 */
const NUMBER_WORDS: Record<string, string> = {
  zero: '0',
  one: '1',
  two: '2',
  three: '3',
  four: '4',
  five: '5',
  six: '6',
  seven: '7',
  eight: '8',
  nine: '9',
  ten: '10',
  eleven: '11',
  twelve: '12',
  thirteen: '13',
  fourteen: '14',
  fifteen: '15',
  sixteen: '16',
  seventeen: '17',
  eighteen: '18',
  nineteen: '19',
  twenty: '20',
  thirty: '30',
  forty: '40',
  fifty: '50',
  sixty: '60',
  seventy: '70',
  eighty: '80',
  ninety: '90',
  hundred: '100',
  thousand: '1000',
  million: '1000000',
  billion: '1000000000',
};

/**
 * Convert number words to digits
 * "one, two, three" → "1, 2, 3"
 * Preserves case-insensitivity and punctuation
 */
export function convertNumberWordsToDigits(text: string): string {
  let result = text;

  // Sort by length descending to match longer words first (e.g., "seventeen" before "seven")
  const sortedWords = Object.keys(NUMBER_WORDS).sort((a, b) => b.length - a.length);

  for (const word of sortedWords) {
    // Match word boundaries, case-insensitive
    const pattern = new RegExp(`\\b${word}\\b`, 'gi');
    result = result.replace(pattern, NUMBER_WORDS[word]);
  }

  return result;
}

/**
 * Utility composable for transcript formatting
 * Handles different output formats including text and JSON
 */
export function useTranscriptFormatter() {
  /**
   * Apply template variables to JSON template
   * Replaces {{variable}} placeholders with actual values
   */
  function applyTemplateVariables(template: string, result: TranscriptionResult): string {
    return template
      .replace(/\{\{id\}\}/g, result.id)
      .replace(/\{\{text\}\}/g, result.text.replace(/"/g, '\\"')) // Escape quotes in text
      .replace(/\{\{timestamp\}\}/g, result.timestamp.toISOString())
      .replace(/\{\{language\}\}/g, result.language || 'en')
      .replace(/\{\{model\}\}/g, result.model || 'unknown')
      .replace(/\{\{duration\}\}/g, String(result.duration || 0));
  }

  /**
   * Format transcription result based on output format
   * @param result The transcription result to format
   * @param format The output format (text, json)
   * @param options Optional formatting options including custom template
   */
  function formatOutput(
    result: TranscriptionResult,
    format: string,
    options?: { removeFillers?: boolean; customTemplate?: string }
  ): string {
    let text = result.text;

    // Apply filler word removal if enabled
    if (options?.removeFillers) {
      text = removeFillerWords(text);
    }

    switch (format) {
      case 'json':
        // Use custom template if provided, otherwise use default JSON
        if (options?.customTemplate) {
          return applyTemplateVariables(options.customTemplate, { ...result, text });
        }
        return JSON.stringify({ ...result, text }, null, 2);
      default:
        return text;
    }
  }

  /**
   * Format time duration as human-readable string
   */
  function formatDuration(seconds: number): string {
    if (seconds < 60) {
      return `${Math.round(seconds)}s`;
    } else if (seconds < 3600) {
      const mins = Math.floor(seconds / 60);
      const secs = Math.round(seconds % 60);
      return `${mins}m ${secs}s`;
    } else {
      const hours = Math.floor(seconds / 3600);
      const mins = Math.floor((seconds % 3600) / 60);
      return `${hours}h ${mins}m`;
    }
  }

  return {
    formatOutput,
    formatDuration,
  };
}
