// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Reusable fade-on-idle composable
 * Handles the common pattern: element fades after idle period, resets on hover
 */

import { writable, type Readable } from 'svelte/store';

export interface FadeOnIdleOptions {
  /** Delay in ms before fading starts (default: 5000) */
  delay?: number;
}

export interface FadeOnIdleState {
  /** Whether the element is currently faded */
  fading: Readable<boolean>;
  /** Whether the element is currently hovered */
  hovered: Readable<boolean>;
}

export interface FadeOnIdleActions {
  /** Start the fade timer (call when element becomes visible or content changes) */
  startFade: () => void;
  /** Call on mouseenter - cancels fade and shows element */
  handleEnter: () => void;
  /** Call on mouseleave - restarts fade timer */
  handleLeave: () => void;
  /** Reset to initial state (not fading, not hovered) */
  reset: () => void;
  /** Cleanup timeouts (call in onDestroy) */
  cleanup: () => void;
}

export type FadeOnIdle = FadeOnIdleState & FadeOnIdleActions;

/**
 * Creates a fade-on-idle controller for UI elements that should fade after inactivity
 *
 * @example
 * ```svelte
 * <script>
 *   const fade = useFadeOnIdle({ delay: 3000 });
 *   $: if (contentChanged) fade.startFade();
 * </script>
 *
 * <button
 *   on:mouseenter={fade.handleEnter}
 *   on:mouseleave={fade.handleLeave}
 *   class:opacity-40={$fade.fading && !$fade.hovered}
 * >
 *   Content
 * </button>
 * ```
 */
export function useFadeOnIdle(options: FadeOnIdleOptions = {}): FadeOnIdle {
  const { delay = 5000 } = options;

  const fading = writable(false);
  const hovered = writable(false);
  let fadeTimeout: ReturnType<typeof setTimeout> | null = null;

  function clearFadeTimeout() {
    if (fadeTimeout) {
      clearTimeout(fadeTimeout);
      fadeTimeout = null;
    }
  }

  function startFade() {
    fading.set(false);
    clearFadeTimeout();

    fadeTimeout = setTimeout(() => {
      // Only fade if not currently hovered
      hovered.subscribe(isHovered => {
        if (!isHovered) {
          fading.set(true);
        }
      })();
    }, delay);
  }

  function handleEnter() {
    hovered.set(true);
    fading.set(false);
    clearFadeTimeout();
  }

  function handleLeave() {
    hovered.set(false);
    // Restart fade timer when mouse leaves
    startFade();
  }

  function reset() {
    fading.set(false);
    hovered.set(false);
    clearFadeTimeout();
  }

  function cleanup() {
    clearFadeTimeout();
  }

  return {
    fading: { subscribe: fading.subscribe },
    hovered: { subscribe: hovered.subscribe },
    startFade,
    handleEnter,
    handleLeave,
    reset,
    cleanup,
  };
}
