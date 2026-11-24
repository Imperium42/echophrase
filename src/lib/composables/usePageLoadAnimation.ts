// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Page load animation state management
 * Staggered fade-in: center first, then corners cascade outward
 */

import { writable, derived } from 'svelte/store';

// Animation timing constants (ms) - SYNCED WITH PULSE
const BUTTON_FADE_DELAY = 100; // Delay for GPU layer warmup before button starts
const SONAR_START_DELAY = 350; // Sonar starts after button is visible
const CORNER_START_DELAY = 650; // Corners start right after pulse ends (350 + 600 = 950ms from start)
const CORNER_STAGGER = 120; // 120ms between each corner

// Core state
const buttonReady = writable(false); // Just the record button
const sonarActive = writable(false);
const brandingReady = writable(false); // Logo + title
const subtitleReady = writable(false); // Subtitle (slightly delayed)

// Staggered corner states
const topRightReady = writable(false);
const bottomLeftReady = writable(false);
const bottomCenterReady = writable(false);
const versionReady = writable(false);

// Banner entrance - fires after entire animation sequence completes
const bannersReady = writable(false);

// Derived state
export const cornersReady = derived(
  [topRightReady, bottomLeftReady, bottomCenterReady, versionReady],
  ([$tr, $bl, $bc, $v]) => $tr && $bl && $bc && $v
);

export {
  buttonReady,
  sonarActive,
  brandingReady,
  subtitleReady,
  topRightReady,
  bottomLeftReady,
  bottomCenterReady,
  versionReady,
  bannersReady,
};

/**
 * Initialize the page load animation sequence
 *
 * Timeline (synced with pulse):
 * 0ms     - Button appears (alone)
 * 400ms   - Pulse + Branding start together (both 600ms)
 * 1000ms  - Pulse + Branding complete
 * 1050ms  - Top-right corner fades in
 * 1170ms  - Bottom-left corner fades in
 * 1290ms  - Bottom-center fades in
 * 1410ms  - Version indicator fades in
 */
export function startPageLoadAnimation(): void {
  // Reset any stale state from previous load (F5 reload)
  resetPageLoadAnimation();

  // Step 1: Record button appears first (alone)
  setTimeout(() => {
    buttonReady.set(true);
  }, BUTTON_FADE_DELAY);

  // Step 2: Pulse starts
  setTimeout(() => {
    sonarActive.set(true);
  }, BUTTON_FADE_DELAY + SONAR_START_DELAY);

  // Step 3: Logo+title starts slightly after pulse (150ms delay)
  setTimeout(
    () => {
      brandingReady.set(true);
    },
    BUTTON_FADE_DELAY + SONAR_START_DELAY + 150
  );

  // Step 4: Subtitle fades in after branding container is fully opaque
  // Branding starts at +150ms, container duration-600 = done at +750ms, then +100ms gap
  setTimeout(
    () => {
      subtitleReady.set(true);
    },
    BUTTON_FADE_DELAY + SONAR_START_DELAY + 850
  );

  // Step 5: Corners cascade in after pulse completes
  setTimeout(
    () => {
      topRightReady.set(true);

      setTimeout(() => {
        bottomLeftReady.set(true);

        setTimeout(() => {
          bottomCenterReady.set(true);

          setTimeout(() => {
            versionReady.set(true);

            // Step 6: Banners slide in after all corners are done
            setTimeout(() => {
              bannersReady.set(true);
            }, 200);
          }, CORNER_STAGGER);
        }, CORNER_STAGGER);
      }, CORNER_STAGGER);
    },
    BUTTON_FADE_DELAY + SONAR_START_DELAY + CORNER_START_DELAY
  );
}

/**
 * Reset animation state
 */
export function resetPageLoadAnimation(): void {
  buttonReady.set(false);
  sonarActive.set(false);
  brandingReady.set(false);
  subtitleReady.set(false);
  topRightReady.set(false);
  bottomLeftReady.set(false);
  bottomCenterReady.set(false);
  versionReady.set(false);
  bannersReady.set(false);
}
