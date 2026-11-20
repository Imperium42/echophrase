// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Animated time countdown display with smooth lerp transitions
 * Used for daily limit countdown that animates when time is deducted
 */

import { writable, type Readable } from 'svelte/store';

export interface TimeAnimationState {
  /** The current display value (may be mid-animation) */
  displaySeconds: Readable<number | null>;
  /** Whether an animation is currently in progress */
  isAnimating: Readable<boolean>;
}

export interface TimeAnimationActions {
  /** Handle a change in the actual time value - animates if decreasing */
  handleChange: (newSeconds: number | null) => void;
  /** Snap immediately to a value without animation */
  snapTo: (seconds: number | null) => void;
  /** Cleanup animation frame (call in onDestroy) */
  cleanup: () => void;
}

export type TimeAnimation = TimeAnimationState & TimeAnimationActions;

/**
 * Creates an animated time display that lerps smoothly when time decreases
 *
 * @example
 * ```svelte
 * <script>
 *   const timeAnim = useTimeAnimation();
 *   $: timeAnim.handleChange($remainingSecondsToday);
 * </script>
 *
 * <span class:text-amber-400={$timeAnim.isAnimating}>
 *   {formatTime($timeAnim.displaySeconds)}
 * </span>
 * ```
 */
export function useTimeAnimation(): TimeAnimation {
  const displaySeconds = writable<number | null>(null);
  const isAnimating = writable(false);

  let previousSeconds: number | null = null;
  let animationFrame: number | null = null;

  /** Ease-out cubic for smooth deceleration */
  function easeOutCubic(t: number): number {
    return 1 - Math.pow(1 - t, 3);
  }

  function cancelAnimation() {
    if (animationFrame) {
      cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }
  }

  function handleChange(newSeconds: number | null): void {
    // Initialize display value on first call
    if (previousSeconds === null && newSeconds !== null) {
      displaySeconds.set(newSeconds);
      previousSeconds = newSeconds;
      return;
    }

    // Detect deduction (current < previous, both non-null, previous > 0)
    if (
      newSeconds !== null &&
      previousSeconds !== null &&
      newSeconds < previousSeconds &&
      previousSeconds > 0
    ) {
      // Start the deduction animation
      isAnimating.set(true);
      const startValue = previousSeconds;
      const endValue = newSeconds;
      const duration = 1000; // 1 second lerp
      const startTime = performance.now();

      cancelAnimation();

      function animateLerp(currentTime: number) {
        const elapsed = currentTime - startTime;
        const progress = Math.min(elapsed / duration, 1);
        const easedProgress = easeOutCubic(progress);

        displaySeconds.set(Math.round(startValue - (startValue - endValue) * easedProgress));

        if (progress < 1) {
          animationFrame = requestAnimationFrame(animateLerp);
        } else {
          // Animation complete
          displaySeconds.set(endValue);
          isAnimating.set(false);
          animationFrame = null;
        }
      }

      animationFrame = requestAnimationFrame(animateLerp);
    } else if (newSeconds !== null) {
      // Non-deduction update (e.g., initial load, refresh) - snap to value
      let currentIsAnimating = false;
      isAnimating.subscribe(v => (currentIsAnimating = v))();
      if (!currentIsAnimating) {
        displaySeconds.set(newSeconds);
      }
    }

    previousSeconds = newSeconds;
  }

  function snapTo(seconds: number | null): void {
    cancelAnimation();
    isAnimating.set(false);
    displaySeconds.set(seconds);
    previousSeconds = seconds;
  }

  function cleanup(): void {
    cancelAnimation();
  }

  return {
    displaySeconds: { subscribe: displaySeconds.subscribe },
    isAnimating: { subscribe: isAnimating.subscribe },
    handleChange,
    snapTo,
    cleanup,
  };
}
