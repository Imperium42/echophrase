<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Setting Slider (shared primitive)
   *
   * The house slider: lime gradient fill + round thumb on a dark track,
   * grab/grabbing cursor, drag + click + keyboard support. Extracted from
   * AudioInputSection's threshold slider so every settings slider looks
   * and behaves the same (DRY).
   *
   * - `oninput` fires continuously during drag (no persistence expected)
   * - `oncommit` fires once on release / click / keyboard change - persist here
   */

  interface RampStop {
    /** Position along the track, 0-100. */
    at: number;
    /** Hex color at that position. */
    color: string;
  }

  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    ariaLabel: string;
    disabled?: boolean;
    /** Optional labels rendered under the track (e.g. ['0%', '50%', '100%']) */
    labels?: string[];
    /**
     * Percent positions for `labels`. Omit for even spacing (the default).
     * Needed when the scale is non-linear, so a label sits over its real tick.
     */
    labelPositions?: number[];
    /**
     * 'lime' (default) is the flat house fill. 'ramp' paints `rampStops` across
     * the FULL track and clips it to the fill, so a given position always shows
     * the same color instead of the gradient stretching as the value changes.
     */
    variant?: 'lime' | 'ramp';
    rampStops?: RampStop[];
    oninput?: (value: number) => void;
    oncommit?: (value: number) => void;
  }

  let {
    value,
    min = 0,
    max = 100,
    step = 1,
    ariaLabel,
    disabled = false,
    labels = [],
    labelPositions,
    variant = 'lime',
    rampStops = [],
    oninput,
    oncommit,
  }: Props = $props();

  let isDragging = $state(false);
  let sliderElement = $state<HTMLDivElement | null>(null);

  const percent = $derived(((value - min) / (max - min)) * 100);

  const rampCss = $derived(
    rampStops.length > 0
      ? `linear-gradient(90deg, ${rampStops.map(s => `${s.color} ${s.at}%`).join(', ')})`
      : 'none'
  );

  /** Interpolate the ramp at `percent` so the thumb matches the track under it. */
  function sampleRamp(pct: number): string {
    if (rampStops.length === 0) return '#84cc16';
    const sorted = [...rampStops].sort((a, b) => a.at - b.at);
    if (pct <= sorted[0].at) return sorted[0].color;
    const last = sorted[sorted.length - 1];
    if (pct >= last.at) return last.color;

    const hi = sorted.findIndex(s => s.at >= pct);
    const a = sorted[hi - 1];
    const b = sorted[hi];
    const ratio = b.at === a.at ? 0 : (pct - a.at) / (b.at - a.at);

    const parse = (hex: string) => [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16));
    const [ar, ag, ab] = parse(a.color);
    const [br, bg, bb] = parse(b.color);
    const mix = (x: number, y: number) => Math.round(x + (y - x) * ratio);
    return `rgb(${mix(ar, br)}, ${mix(ag, bg)}, ${mix(ab, bb)})`;
  }

  const thumbColor = $derived(variant === 'ramp' ? sampleRamp(percent) : '#84cc16');

  function snap(raw: number): number {
    const stepped = Math.round((raw - min) / step) * step + min;
    return Math.max(min, Math.min(max, stepped));
  }

  // Cache rect on drag start to avoid getBoundingClientRect() on every mousemove (forced reflow)
  let cachedRect: DOMRect | null = null;
  let dragValue = 0;

  function handleDrag(e: MouseEvent | TouchEvent) {
    if (!cachedRect) return;
    const clientX = 'touches' in e ? e.touches[0].clientX : e.clientX;
    const ratio = Math.max(0, Math.min(1, (clientX - cachedRect.left) / cachedRect.width));
    dragValue = snap(min + ratio * (max - min));
    oninput?.(dragValue);
  }

  function startDrag(e: MouseEvent | TouchEvent) {
    if (disabled) return;
    isDragging = true;
    dragValue = value;
    if (sliderElement) cachedRect = sliderElement.getBoundingClientRect();
    handleDrag(e);
    window.addEventListener('mousemove', handleDrag);
    window.addEventListener('mouseup', stopDrag);
    window.addEventListener('touchmove', handleDrag);
    window.addEventListener('touchend', stopDrag);
  }

  function stopDrag() {
    cachedRect = null;
    if (isDragging) {
      isDragging = false;
      oncommit?.(dragValue);
    }
    window.removeEventListener('mousemove', handleDrag);
    window.removeEventListener('mouseup', stopDrag);
    window.removeEventListener('touchmove', handleDrag);
    window.removeEventListener('touchend', stopDrag);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (disabled) return;
    let next: number;
    if (e.key === 'ArrowRight' || e.key === 'ArrowUp') next = snap(value + step);
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') next = snap(value - step);
    else if (e.key === 'Home') next = min;
    else if (e.key === 'End') next = max;
    else return;
    e.preventDefault();
    oninput?.(next);
    oncommit?.(next);
  }
</script>

<div
  class="setting-slider"
  class:dragging={isDragging}
  class:disabled
  bind:this={sliderElement}
  onmousedown={startDrag}
  ontouchstart={startDrag}
  onkeydown={handleKeydown}
  role="slider"
  aria-label={ariaLabel}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-disabled={disabled}
  tabindex={disabled ? -1 : 0}
>
  <div class="slider-track">
    {#if variant === 'ramp'}
      <!-- Full-width gradient, clipped to the fill: color stays pinned to
           position rather than stretching with the value. -->
      <div
        class="slider-fill ramp"
        style="background-image: {rampCss}; clip-path: inset(0 {100 - percent}% 0 0)"
      ></div>
    {:else}
      <div class="slider-fill" style="width: {percent}%"></div>
    {/if}
    <div class="slider-thumb" style="left: {percent}%; --thumb-color: {thumbColor}"></div>
  </div>
  {#if labels.length > 0}
    <div class="slider-labels" class:positioned={labelPositions != null}>
      {#each labels as label, i (label)}
        {#if labelPositions != null}
          <span class="positioned-label" style="left: {labelPositions[i] ?? 0}%">{label}</span>
        {:else}
          <span>{label}</span>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<style>
  .setting-slider {
    position: relative;
    padding: 0.5rem 0 0.75rem;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
    width: 100%;
  }

  .setting-slider.dragging {
    cursor: grabbing;
  }

  .setting-slider.disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .slider-track {
    position: relative;
    height: 6px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 3px;
  }

  .slider-fill {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    background: linear-gradient(90deg, #84cc16, #a3e635);
    border-radius: 3px;
    transition: width 0.05s ease;
  }

  /* Spans the whole track; `clip-path` (set inline) does the revealing. */
  .slider-fill.ramp {
    width: 100%;
    background: none;
    transition: clip-path 0.05s ease;
  }

  .setting-slider.dragging .slider-fill {
    transition: none;
  }

  .slider-thumb {
    position: absolute;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 14px;
    height: 14px;
    background: var(--thumb-color, #84cc16);
    border-radius: 50%;
    box-shadow: 0 0 6px color-mix(in srgb, var(--thumb-color, #84cc16) 40%, transparent);
    /* Only animate left position, not transforms - avoids GPU layer flicker */
    transition:
      left 0.05s ease,
      background 0.15s ease,
      box-shadow 0.15s ease;
  }

  .setting-slider.dragging .slider-thumb {
    box-shadow: 0 0 10px color-mix(in srgb, var(--thumb-color, #84cc16) 60%, transparent);
  }

  .setting-slider:hover:not(.disabled) .slider-thumb {
    box-shadow: 0 0 8px color-mix(in srgb, var(--thumb-color, #84cc16) 50%, transparent);
  }

  .slider-labels {
    display: flex;
    justify-content: space-between;
    font-size: 0.5625rem;
    color: #3f3f46;
    margin-top: 0.5rem;
  }

  /* Non-linear scales place each label over its real tick instead of
     spreading them evenly, which would misreport where the value sits. */
  .slider-labels.positioned {
    display: block;
    position: relative;
    height: 0.75rem;
  }

  .positioned-label {
    position: absolute;
    transform: translateX(-50%);
    white-space: nowrap;
  }

  /* Keep the end labels inside the track rather than overhanging it. */
  .positioned-label[style*='left: 0%'] {
    transform: none;
  }

  .positioned-label[style*='left: 100%'] {
    transform: translateX(-100%);
  }
</style>
