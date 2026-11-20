// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { cubicOut } from 'svelte/easing';
import type { TransitionConfig } from 'svelte/transition';

/** Options for {@link slideFade}. */
export interface SlideFadeParams {
  /** Duration in milliseconds. */
  duration?: number;
  /** Easing applied to the height/spacing collapse. */
  easing?: (t: number) => number;
}

/**
 * `slide`, but the element fades out well before it finishes collapsing.
 *
 * Animating height is a layout operation, so the last frames of a plain slide
 * are where any rounding or reflow hiccup shows up. Driving opacity on a much
 * steeper curve (roughly t^2.5) means the element is visually gone by the time
 * the height is a third of the way down, so the remaining collapse just closes
 * the gap and never draws attention to itself.
 *
 * Reads the computed box up front and scales every vertical dimension together,
 * so borders and margins collapse in step with the content.
 *
 * @param node Element being transitioned.
 * @param params Duration and easing overrides.
 * @returns Svelte transition config.
 */
export function slideFade(
  node: Element,
  { duration = 220, easing = cubicOut }: SlideFadeParams = {}
): TransitionConfig {
  const style: CSSStyleDeclaration = getComputedStyle(node);
  const opacity = parseFloat(style.opacity) || 1;
  const height = parseFloat(style.height);
  const paddingTop = parseFloat(style.paddingTop);
  const paddingBottom = parseFloat(style.paddingBottom);
  const marginTop = parseFloat(style.marginTop);
  const marginBottom = parseFloat(style.marginBottom);
  const borderTopWidth = parseFloat(style.borderTopWidth);
  const borderBottomWidth = parseFloat(style.borderBottomWidth);

  return {
    duration,
    easing,
    css: (t: number) =>
      'overflow: hidden;' +
      `opacity: ${Math.pow(t, 2.5) * opacity};` +
      `height: ${t * height}px;` +
      `padding-top: ${t * paddingTop}px;` +
      `padding-bottom: ${t * paddingBottom}px;` +
      `margin-top: ${t * marginTop}px;` +
      `margin-bottom: ${t * marginBottom}px;` +
      `border-top-width: ${t * borderTopWidth}px;` +
      `border-bottom-width: ${t * borderBottomWidth}px;`,
  };
}
