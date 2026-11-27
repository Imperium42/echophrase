<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<!--
  Floating recording pill (window label `recording_overlay`, see
  src-tauri/src/overlay/mod.rs). State and FFT band levels come from Rust;
  this page only animates them. Concept credit: Handy
  (https://github.com/cjpais/Handy, MIT License, Copyright (c) CJ Pais).
  The look, the mirrored lens waveform, the voice glow and the
  frame-rate-independent spring smoothing are our own.
-->
<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { t, initLocale } from '$lib/locales';

  type OverlayState = 'hidden' | 'recording' | 'transcribing';

  /** Must match `levels::BAND_COUNT` in Rust. */
  const BANDS = 12;
  /** Mirrored: lowest band in the center, highest at both edges. */
  const BARS = BANDS * 2;
  /** Seconds to rise toward a louder target / fall toward a quieter one. */
  const ATTACK_TAU = 0.045;
  const RELEASE_TAU = 0.16;
  /** Smallest bar, as a fraction of full height (keeps a visible baseline). */
  const MIN_SCALE = 0.09;

  let overlayState = $state<OverlayState>('hidden');
  let elapsed = $state(0);
  let bars: HTMLSpanElement[] = $state([]);
  let pill: HTMLDivElement | undefined = $state();

  const target = new Float32Array(BANDS);
  const shown = new Float32Array(BANDS);
  /** Tapers the lens toward the edges so the shape reads as one object. */
  const envelope = Array.from({ length: BANDS }, (_, i) => 1 - 0.55 * (i / (BANDS - 1)) ** 1.6);

  let raf = 0;
  let last = 0;
  let startedAt = 0;
  let unlisteners: UnlistenFn[] = [];

  function applyState(next: OverlayState) {
    if (next === 'recording' && overlayState !== 'recording' && overlayState !== 'transcribing') {
      startedAt = performance.now();
      elapsed = 0;
      target.fill(0);
      shown.fill(0);
    }
    overlayState = next;
    if (next === 'hidden') stopLoop();
    else startLoop();
  }

  function startLoop() {
    if (raf) return;
    last = performance.now();
    raf = requestAnimationFrame(frame);
  }

  function stopLoop() {
    // Keep animating through the exit transition, then park.
    setTimeout(() => {
      if (overlayState === 'hidden' && raf) {
        cancelAnimationFrame(raf);
        raf = 0;
      }
    }, 260);
  }

  function frame(now: number) {
    const dt = Math.min(0.1, (now - last) / 1000);
    last = now;
    const time = now / 1000;

    if (overlayState === 'recording') elapsed = Math.floor((now - startedAt) / 1000);

    let energy = 0;
    for (let i = 0; i < BANDS; i++) {
      // Spatial blur [0.25, 0.5, 0.25] so neighbours move together.
      const l = target[Math.max(0, i - 1)];
      const r = target[Math.min(BANDS - 1, i + 1)];
      let goal = 0.25 * l + 0.5 * target[i] + 0.25 * r;

      if (overlayState === 'transcribing') {
        // A soft pulse travelling outward from the center.
        const wave = Math.max(0, Math.sin(time * 5.2 - i * 0.55));
        goal = 0.12 + 0.42 * wave * wave;
      } else {
        // Gentle idle breathing so silence still looks alive.
        goal = Math.max(goal, 0.05 + 0.035 * (1 + Math.sin(time * 2.4 + i * 0.7)));
      }

      // Time-based EMA (frame-rate independent): fast attack, slow release.
      const tau = goal > shown[i] ? ATTACK_TAU : RELEASE_TAU;
      shown[i] += (goal - shown[i]) * (1 - Math.exp(-dt / tau));
      energy += shown[i];
    }

    for (let b = 0; b < BARS; b++) {
      const band = Math.abs(b - (BARS - 1) / 2) - 0.5;
      const i = Math.min(BANDS - 1, Math.max(0, Math.round(band)));
      const v = MIN_SCALE + (1 - MIN_SCALE) * Math.min(1, shown[i] * envelope[i]);
      const el = bars[b];
      if (el) {
        el.style.transform = `scaleY(${v.toFixed(3)})`;
        el.style.setProperty('--blue', Math.min(1, Math.max(0, (v - 0.45) * 1.8)).toFixed(3));
        el.style.opacity = (0.55 + 0.45 * v).toFixed(3);
      }
    }
    pill?.style.setProperty('--voice', (energy / BANDS).toFixed(3));

    raf = requestAnimationFrame(frame);
  }

  function cancel() {
    invoke('overlay_cancel').catch(e => console.warn('[Overlay] cancel failed:', e));
  }

  function formatElapsed(s: number): string {
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  }

  onMount(async () => {
    document.documentElement.classList.add('dark', 'overlay-root');
    void initLocale().catch(() => {});
    let gotEvent = false;
    unlisteners = await Promise.all([
      listen<OverlayState>('overlay-state', e => {
        gotEvent = true;
        applyState(e.payload);
      }),
      listen<number[]>('overlay-levels', e => {
        const levels = e.payload;
        for (let i = 0; i < BANDS; i++) target[i] = levels[i] ?? 0;
      }),
    ]);
    // The first show can emit before this page has loaded: pull it, unless a
    // newer event already arrived while the request was in flight.
    try {
      const current = await invoke<OverlayState>('overlay_get_state');
      if (!gotEvent) applyState(current);
    } catch (e) {
      console.warn('[Overlay] state pull failed:', e);
    }
  });

  onDestroy(() => {
    for (const unlisten of unlisteners) unlisten();
    if (raf) cancelAnimationFrame(raf);
  });
</script>

<div class="stage">
  <div
    bind:this={pill}
    class="pill"
    class:visible={overlayState !== 'hidden'}
    class:transcribing={overlayState === 'transcribing'}
  >
    <span class="dot" aria-hidden="true"></span>

    <div class="wave" aria-hidden="true">
      {#each Array(BARS) as _, b (b)}
        <span class="bar" bind:this={bars[b]}></span>
      {/each}
    </div>

    <span class="timer">{formatElapsed(elapsed)}</span>

    <button
      class="close"
      type="button"
      aria-label={t('overlay.cancel')}
      disabled={overlayState !== 'recording'}
      onclick={cancel}
    >
      <svg viewBox="0 0 12 12" aria-hidden="true">
        <path d="M3 3l6 6M9 3l-6 6" />
      </svg>
    </button>
  </div>
</div>

<style>
  /* This window is see-through: undo the main window's shell styles. */
  :global(html.overlay-root),
  :global(html.overlay-root body) {
    background: transparent !important;
    min-width: 0 !important;
    min-height: 0 !important;
    overflow: hidden;
    user-select: none;
  }

  .stage {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
  }

  .pill {
    --voice: 0;
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 6px 0 14px;
    border-radius: 999px;
    background:
      linear-gradient(180deg, oklch(1 0 0 / 6%), oklch(1 0 0 / 0%) 55%),
      color-mix(in oklch, var(--card) 92%, black);
    border: 1px solid oklch(1 0 0 / 10%);
    box-shadow:
      inset 0 1px 0 oklch(1 0 0 / 7%),
      0 8px 22px -6px oklch(0 0 0 / 60%),
      0 2px 6px oklch(0 0 0 / 35%);
    opacity: 0;
    transform: translateY(10px) scale(0.94);
    filter: blur(3px);
    transition:
      opacity 180ms ease-in,
      transform 200ms ease-in,
      filter 200ms ease-in;
  }

  .pill.visible {
    opacity: 1;
    transform: none;
    filter: none;
    transition:
      opacity 260ms ease-out,
      transform 460ms cubic-bezier(0.22, 1, 0.36, 1),
      filter 300ms ease-out;
  }

  /* Voice glow: a violet halo under the pill that swells with loudness. */
  .pill::before {
    content: '';
    position: absolute;
    inset: 6px 30px -4px;
    z-index: -1;
    border-radius: inherit;
    background: var(--color-overlay-accent);
    filter: blur(14px);
    opacity: calc(0.08 + var(--voice) * 0.5);
    transform: scaleX(calc(0.7 + var(--voice) * 0.5));
  }

  .dot {
    position: relative;
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
    /* Rose while listening, brand violet while transcribing. */
    --dot-color: var(--color-overlay-record);
    background: var(--dot-color);
    box-shadow: 0 0 8px color-mix(in oklch, var(--dot-color) 70%, transparent);
    transition:
      background 240ms ease,
      box-shadow 240ms ease;
  }

  .dot::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    border: 1.5px solid var(--dot-color);
    animation: ring 1.6s cubic-bezier(0.22, 1, 0.36, 1) infinite;
  }

  .transcribing .dot {
    --dot-color: var(--color-overlay-accent);
    animation: breathe 1.1s ease-in-out infinite;
  }

  .transcribing .dot::after {
    animation: none;
    opacity: 0;
  }

  .wave {
    display: flex;
    align-items: center;
    gap: 2.5px;
    height: 24px;
  }

  .bar {
    width: 3px;
    height: 24px;
    border-radius: 3px;
    position: relative;
    background: linear-gradient(
      0deg,
      var(--color-overlay-accent),
      var(--color-overlay-accent) 50%,
      color-mix(in oklch, var(--color-overlay-accent) 72%, white)
    );
    transform: scaleY(0.09);
    transform-origin: center;
  }

  /* Blue lift from the bottom; its opacity (--blue, set per frame in JS) is 0
     until a bar passes ~45% height. A plain opacity layer, not calc() inside
     color-mix(), so older WebKit renders it too. */
  .bar::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: linear-gradient(0deg, var(--color-overlay-accent-low), transparent 60%);
    opacity: var(--blue, 0);
  }

  .timer {
    min-width: 30px;
    font-size: 11.5px;
    font-weight: 500;
    letter-spacing: 0.02em;
    font-variant-numeric: tabular-nums;
    color: var(--muted-foreground);
    text-align: right;
    transition: opacity 200ms ease;
  }

  .transcribing .timer {
    opacity: 0.45;
  }

  .close {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    flex: none;
    border-radius: 50%;
    color: var(--muted-foreground);
    background: oklch(1 0 0 / 5%);
    cursor: pointer;
    transition:
      background 140ms ease,
      color 140ms ease,
      transform 140ms ease,
      opacity 200ms ease;
  }

  .close svg {
    width: 10px;
    height: 10px;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
  }

  .close:hover {
    color: var(--foreground);
    background: oklch(0.65 0.2 25 / 35%);
  }

  .close:active {
    transform: scale(0.9);
  }

  .close:disabled {
    opacity: 0;
    pointer-events: none;
  }

  @keyframes ring {
    0% {
      transform: scale(1);
      opacity: 0.7;
    }
    100% {
      transform: scale(2.6);
      opacity: 0;
    }
  }

  @keyframes breathe {
    50% {
      opacity: 0.45;
      transform: scale(0.8);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .dot::after,
    .transcribing .dot {
      animation: none;
    }
    .pill,
    .pill.visible {
      transform: none;
      filter: none;
      transition: opacity 160ms linear;
    }
  }
</style>
