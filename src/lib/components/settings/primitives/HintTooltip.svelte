<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Hint Tooltip
   *
   * Small question-mark icon that reveals a tooltip on hover.
   * Shared primitive - use this instead of inlining the SVG + CSS everywhere.
   *
   * The bubble is positioned in JS (viewport-fixed, clamped to stay on
   * screen) rather than CSS `position: absolute` relative to the trigger -
   * an absolute bubble gets silently clipped whenever the trigger sits
   * inside a scrolling container (eg a candidate list with
   * `overflow-y: auto`), which cuts the text off mid-sentence instead of
   * showing it.
   *
   * The bubble is additionally PORTALED to <body>. `position: fixed` alone
   * is not enough: any ancestor with `contain` (ModalWindow's backdrop uses
   * `contain: layout style paint`), `transform`, or `filter` becomes the
   * containing block, re-introducing clipping inside scrollers - this is
   * exactly what truncated the HF variant hints mid-sentence.
   *
   * Usage:
   *   <HintTooltip hint="This is the explanation" />
   */

  let { hint }: { hint: string } = $props();

  /** Svelte action: move the element to <body> so no ancestor can clip it. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }

  let triggerEl: HTMLElement | undefined = $state();
  let tooltipEl: HTMLElement | undefined = $state();
  let visible = $state(false);
  let top = $state(0);
  let left = $state(0);
  let arrowOffset = $state(0);
  let above = $state(false);

  const EDGE_MARGIN = 8;

  function position() {
    if (!triggerEl || !tooltipEl) return;
    const trigger = triggerEl.getBoundingClientRect();
    const tooltipWidth = tooltipEl.offsetWidth;

    const idealLeft = trigger.left + trigger.width / 2 - tooltipWidth / 2;
    const clampedLeft = Math.min(
      Math.max(idealLeft, EDGE_MARGIN),
      window.innerWidth - tooltipWidth - EDGE_MARGIN
    );

    left = clampedLeft;
    // Open below; flip above when there's no room at the bottom of the window.
    const below = trigger.bottom + 6;
    above = below + tooltipEl.offsetHeight + EDGE_MARGIN > window.innerHeight;
    top = above ? trigger.top - 6 - tooltipEl.offsetHeight : below;
    // Keep the arrow pointing at the trigger even when the bubble itself
    // had to shift to stay on screen.
    arrowOffset = trigger.left + trigger.width / 2 - clampedLeft;
  }

  function show() {
    visible = true;
    // Bubble must render (and have a measurable width) before positioning.
    requestAnimationFrame(position);
  }

  function hide() {
    visible = false;
  }
</script>

<span
  class="hint-trigger"
  bind:this={triggerEl}
  onmouseenter={show}
  onmouseleave={hide}
  onfocus={show}
  onblur={hide}
  onclick={e => e.stopPropagation()}
  onkeydown={e => {
    if (e.key === 'Enter' || e.key === ' ') e.stopPropagation();
  }}
  role="button"
  aria-label={hint}
  tabindex="0"
>
  <svg class="hint-icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
    <path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z" />
    <path
      d="M5.255 5.786a.237.237 0 0 0 .241.247h.825c.138 0 .248-.113.266-.25.09-.656.54-1.134 1.342-1.134.686 0 1.314.343 1.314 1.168 0 .635-.374.927-.965 1.371-.673.489-1.206 1.06-1.168 1.987l.003.217a.25.25 0 0 0 .25.246h.811a.25.25 0 0 0 .25-.25v-.105c0-.718.273-.927 1.01-1.486.609-.463 1.244-.977 1.244-2.056 0-1.511-1.276-2.241-2.673-2.241-1.267 0-2.655.59-2.75 2.286zm1.557 5.763c0 .533.425.927 1.01.927.609 0 1.028-.394 1.028-.927 0-.552-.42-.94-1.029-.94-.584 0-1.009.388-1.009.94z"
    />
  </svg>
  <span
    class="hint-tooltip"
    class:visible
    class:above
    bind:this={tooltipEl}
    use:portal
    style="top: {top}px; left: {left}px; --arrow-offset: {arrowOffset}px;"
  >
    {hint}
  </span>
</span>

<style>
  .hint-trigger {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: help;
  }

  .hint-icon {
    width: 0.75rem;
    height: 0.75rem;
    color: var(--color-hint);
    transition: color 0.15s ease;
  }

  .hint-trigger:hover .hint-icon {
    color: var(--color-hint-hover);
  }

  /* Fixed to the viewport (not the trigger) so it can never be clipped by
	   a scrolling ancestor - position comes from JS, not CSS offsets. */
  .hint-tooltip {
    position: fixed;
    padding: 0.5rem 0.75rem;
    background: rgba(39, 39, 42, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.375rem;
    font-size: 0.6875rem;
    font-weight: 400;
    color: #a1a1aa;
    white-space: normal;
    max-width: 280px;
    min-width: 180px;
    text-align: center;
    line-height: 1.5;
    opacity: 0;
    visibility: hidden;
    transition:
      opacity 0.15s ease,
      visibility 0.15s ease;
    pointer-events: none;
    z-index: 1000;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .hint-tooltip::after {
    content: '';
    position: absolute;
    bottom: 100%;
    left: var(--arrow-offset, 50%);
    transform: translateX(-50%);
    border: 5px solid transparent;
    border-bottom-color: rgba(39, 39, 42, 0.95);
  }

  .hint-tooltip.above::after {
    bottom: auto;
    top: 100%;
    border-bottom-color: transparent;
    border-top-color: rgba(39, 39, 42, 0.95);
  }

  .hint-tooltip.visible {
    opacity: 1;
    visibility: visible;
  }
</style>
