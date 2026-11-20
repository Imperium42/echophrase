// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Realtime transcription preview composable
 * Handles live preview with word-by-word animation and pagination
 */

import { writable, type Readable, get } from 'svelte/store';
import { removeFillerWords, convertNumberWordsToDigits } from './useTranscriptFormatter';

const WORDS_PER_PAGE = 30;

export interface RealtimePreviewState {
  /** The current page of text to display */
  text: Readable<string>;
  /** Whether the preview is visible */
  isVisible: Readable<boolean>;
  /** Whether the preview is fading out */
  isFading: Readable<boolean>;
  /** Current page index (0-based) */
  pageIndex: Readable<number>;
  /** Whether currently showing original text before transform */
  isShowingOriginal: Readable<boolean>;
  /** Whether transform animation is in progress (blur/fade) */
  isTransforming: Readable<boolean>;
  /** True when the current segment is the backend's silence sentinel
   * (near-silent audio or a known ASR hallucination on silence). The
   * consuming component renders a dedicated "no sound detected" message
   * instead of the raw sentinel text. */
  isSilenceResult: Readable<boolean>;
}

export interface TransformInfo {
  isTransformed?: boolean;
  isOriginalBeforeTransform?: boolean;
  originalText?: string;
  /** Set by the caller when `segmentText` is the backend's silence sentinel. */
  isSilence?: boolean;
}

export interface RealtimePreviewActions {
  /** Handle an incoming transcription segment */
  handleSegment: (
    text: string,
    isFinal: boolean,
    settings: { remove_filler_words?: boolean; use_numeric_digits?: boolean } | null,
    transformInfo?: TransformInfo
  ) => void;
  /** Reset preview state (call when starting new recording) */
  reset: () => void;
  /** Cleanup timeouts (call in onDestroy) */
  cleanup: () => void;
}

export type RealtimePreview = RealtimePreviewState & RealtimePreviewActions;

/**
 * Creates a realtime preview controller for live transcription display
 *
 * @example
 * ```svelte
 * <script>
 *   const preview = useRealtimePreview();
 *
 *   // In event listener
 *   preview.handleSegment(text, isFinal, $tauriSettings);
 *
 *   // When starting new recording
 *   preview.reset();
 * </script>
 *
 * {#if $preview.isVisible}
 *   <div class:opacity-0={$preview.isFading}>
 *     {$preview.text}
 *   </div>
 * {/if}
 * ```
 */
export function useRealtimePreview(): RealtimePreview {
  const text = writable('');
  const isVisible = writable(false);
  const isFading = writable(false);
  const pageIndex = writable(0);
  const isShowingOriginal = writable(false);
  const isTransforming = writable(false);
  const isSilenceResult = writable(false);

  let fadeTimeout: ReturnType<typeof setTimeout> | null = null;
  let pageLinger: ReturnType<typeof setTimeout> | null = null;
  let transformTimeout: ReturnType<typeof setTimeout> | null = null;
  let isPaginating = false;

  function clearTimeouts() {
    if (fadeTimeout) {
      clearTimeout(fadeTimeout);
      fadeTimeout = null;
    }
    if (pageLinger) {
      clearTimeout(pageLinger);
      pageLinger = null;
    }
    if (transformTimeout) {
      clearTimeout(transformTimeout);
      transformTimeout = null;
    }
  }

  function handleSegment(
    segmentText: string,
    isFinal: boolean,
    settings: { remove_filler_words?: boolean; use_numeric_digits?: boolean } | null,
    transformInfo?: TransformInfo
  ): void {
    // Clear any existing fade timeout
    if (fadeTimeout) {
      clearTimeout(fadeTimeout);
      fadeTimeout = null;
    }

    // Handle transform animation states
    if (transformInfo?.isOriginalBeforeTransform) {
      // Showing original text - set the flag
      isShowingOriginal.set(true);
      isTransforming.set(false);
    } else if (transformInfo?.isTransformed) {
      // Transform is happening - trigger animation
      isTransforming.set(true);
      // After animation completes, clear flags
      transformTimeout = setTimeout(() => {
        isShowingOriginal.set(false);
        isTransforming.set(false);
      }, 600); // Animation duration
    } else {
      // Normal text, no transform
      isShowingOriginal.set(false);
      isTransforming.set(false);
    }

    isSilenceResult.set(transformInfo?.isSilence === true);

    // Apply the same text transformations as final output. Skipped for the
    // silence sentinel - filler-word/digit cleanup has nothing to do there,
    // and the consuming component renders its own message for this case
    // instead of `text` anyway.
    let processedText = segmentText;

    if (!transformInfo?.isSilence) {
      // Apply filler word removal if enabled (default: true)
      if (settings?.remove_filler_words !== false) {
        processedText = removeFillerWords(processedText);
      }

      // Apply number-to-digit conversion if enabled
      if (settings?.use_numeric_digits === true) {
        processedText = convertNumberWordsToDigits(processedText);
      }
    }

    // Pagination logic: show current page of words
    const allWords = processedText
      .trim()
      .split(' ')
      .filter((w: string) => w);
    const currentPageIndex = get(pageIndex);
    const pageStart = currentPageIndex * WORDS_PER_PAGE;
    const pageEnd = pageStart + WORDS_PER_PAGE;
    const currentPageWords = allWords.slice(pageStart, pageEnd);

    // Always update displayed text for current page (unless mid-fade)
    const currentIsFading = get(isFading);
    if (!currentIsFading) {
      text.set(currentPageWords.join(' '));
    }

    // Check if we need to paginate (current page is full and more words exist)
    if (allWords.length > pageEnd && !isPaginating) {
      // Start pagination: linger, fade out, advance page
      isPaginating = true;

      // Cancel any existing linger timeout
      if (pageLinger) clearTimeout(pageLinger);

      pageLinger = setTimeout(() => {
        // Fade out
        isFading.set(true);

        setTimeout(() => {
          // Advance to next page
          pageIndex.update(n => n + 1);
          isFading.set(false);
          isPaginating = false;

          // Update display with new page
          const newPageIndex = get(pageIndex);
          const newPageStart = newPageIndex * WORDS_PER_PAGE;
          const newPageWords = allWords.slice(newPageStart, newPageStart + WORDS_PER_PAGE);
          text.set(newPageWords.join(' '));
        }, 300); // Fade duration
      }, 1500); // Linger duration
    }

    isVisible.set(true);
    if (!isPaginating) isFading.set(false);

    // After final segment, keep visible for 10s then fade out
    if (isFinal) {
      // Cancel any pagination in progress
      if (pageLinger) clearTimeout(pageLinger);
      isPaginating = false;

      fadeTimeout = setTimeout(() => {
        isFading.set(true);
        // After fade animation (500ms), hide completely
        setTimeout(() => {
          isVisible.set(false);
          text.set('');
          isFading.set(false);
          pageIndex.set(0); // Reset for next recording
        }, 500);
      }, 10000);
    }
  }

  function reset(): void {
    clearTimeouts();
    text.set('');
    isVisible.set(false);
    isFading.set(false);
    pageIndex.set(0);
    isShowingOriginal.set(false);
    isTransforming.set(false);
    isSilenceResult.set(false);
    isPaginating = false;
  }

  function cleanup(): void {
    clearTimeouts();
  }

  return {
    text: { subscribe: text.subscribe },
    isVisible: { subscribe: isVisible.subscribe },
    isFading: { subscribe: isFading.subscribe },
    pageIndex: { subscribe: pageIndex.subscribe },
    isShowingOriginal: { subscribe: isShowingOriginal.subscribe },
    isTransforming: { subscribe: isTransforming.subscribe },
    isSilenceResult: { subscribe: isSilenceResult.subscribe },
    handleSegment,
    reset,
    cleanup,
  };
}
