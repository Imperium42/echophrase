<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Output Toasts Container
   *
   * Left-of-button region:
   *  - Active output confirmation toasts (auto-dismiss, hover-pause).
   *  - Last-toast half-fade on area hover.
   *  - Amber "last run" action-chip stack driven by $lastRun.
   *    Chips summarize the most recent recording's actions/stats - not transcript text.
   *    Hovering a chip projects its content onto the right-side preview surface.
   */
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import { mustInvoke } from '$lib/ipc/invoke';
  import { outputToasts, scheduleToastDismiss, type OutputToast } from '$lib/stores/outputToasts';
  import { lastRun, lastRuns, historyPeekRequested } from '$lib/stores/lastRun';
  import { demoChipsPinned } from '$lib/stores/demo';
  import { rightProjection } from '$lib/stores/rightProjection';
  import { useCopyFeedback } from '$lib/composables/useCopyFeedback';
  import { t } from '$lib/locales';
  import { Icon, type IconName } from '$lib/icons';
  import OutputToastItem from './OutputToastItem.svelte';
  import {
    hasSeenHistoryPeek,
    markHistoryPeekSeen,
    resetHistoryPeekSeen,
  } from '$lib/utils/historyPeek';
  import {
    stepHistory,
    historyLabel,
    pagerCaption,
    type PagerArrow,
  } from '$lib/utils/historyPager';

  /** History hint: once per app session, after a run and 5s of downtime, fade
   * in a gold "Hover for history" chevron pointing at this area, then fade it
   * out. Hovering the area dismisses it for the session. */
  let peekShown: boolean = typeof window !== 'undefined' && hasSeenHistoryPeek();
  let peekActive = $state(false);
  /** How long the hint stays up once shown. */
  const HINT_SHOW_MS = 3500;
  /** Debug "History Hint" button: shows the hint now, even with no run yet. */
  let hintPreview = $state(false);
  let hintPreviewTimer: ReturnType<typeof setTimeout> | null = null;
  let peekInTimer: ReturnType<typeof setTimeout> | null = null;
  let peekOutTimer: ReturnType<typeof setTimeout> | null = null;

  type Props = {
    /** Callback when the left chip surface is shown/hidden */
    onHistoryHoverChange?: (hovering: boolean) => void;
  };

  let { onHistoryHoverChange = undefined }: Props = $props();

  /** Whether the left area is being hovered */
  let areaHovered = $state(false);

  /** Which chip (by key) just flashed "Copied" feedback */
  let copiedChipKey = $state<string | null>(null);
  const rowCopyFeedback = useCopyFeedback({ feedbackDuration: 1200 });

  let activeToasts = $derived($outputToasts);

  /** Pager position in $lastRuns: 0 = latest run, higher = older. */
  let historyIndex = $state(0);
  /** Which pager arrow is hovered/focused, for its "Older"/"Newer" caption. */
  let pagerHover = $state<PagerArrow | null>(null);

  // Null while recording (beginRun clears $lastRun) so the chips still hide.
  // Index 0 is $lastRun itself (it can be the demo's run, which is not in the
  // history); older indexes come from the history.
  let run = $derived(
    $lastRun === null || historyIndex === 0 ? $lastRun : ($lastRuns[historyIndex] ?? $lastRun)
  );

  let canNewer = $derived(historyIndex > 0);
  let canOlder = $derived(historyIndex < $lastRuns.length - 1);

  let pagerCaptionKey = $derived(pagerCaption(historyIndex, $lastRuns.length, pagerHover));

  function stepPager(delta: number): void {
    historyIndex = stepHistory(historyIndex, delta, $lastRuns.length);
  }

  interface Chip {
    key: string;
    label: string;
    icon: IconName;
    text?: string;
    clickable: boolean;
  }

  let chips = $derived.by((): Chip[] => {
    if (!run) return [];
    const out: Chip[] = [];
    out.push({
      key: 'raw',
      label: t('lastRun.rawTranscript'),
      icon: 'file-text',
      text: run.raw,
      clickable: true,
    });
    if (run.transformed && run.transformed !== run.raw) {
      out.push({
        key: 'transformed',
        label: t('toast.transformed'),
        icon: 'sparkles',
        text: run.transformed,
        clickable: true,
      });
    }
    for (const a of run.actions) {
      if (a === 'copy_to_clipboard')
        out.push({
          key: 'copied',
          label: t('toast.copied'),
          icon: 'clipboard',
          text: run.output,
          clickable: !!run.output,
        });
      if (a === 'paste_at_cursor')
        out.push({
          key: 'pasted',
          label: t('toast.pasted'),
          icon: 'paste',
          text: run.output,
          clickable: !!run.output,
        });
      if (a === 'save_as_file')
        out.push({
          key: 'saved',
          label: t('toast.saved'),
          icon: 'save',
          text: run.output,
          clickable: !!run.output,
        });
    }
    return out;
  });

  /** Speed stats for the shown run, on the pager row (not a chip - the stack got crowded). */
  let speedLabel = $derived.by((): string | null => {
    const s = run?.speed;
    if (!s) return null;
    return s.transformSeconds !== null
      ? `${formatSeconds(s.transcriptionSeconds)} · ${formatSeconds(s.transformSeconds)} · ${formatSeconds(s.totalSeconds)}`
      : formatSeconds(s.transcriptionSeconds);
  });

  let showPager = $derived($lastRuns.length > 1 && !$demoChipsPinned);

  /** Seconds with 1 decimal; sub-10ms values (only the scripted demo) render as ms. */
  function formatSeconds(seconds: number): string {
    return seconds < 0.01 ? `${(seconds * 1000).toFixed(1)}ms` : `${seconds.toFixed(1)}s`;
  }

  let showChips = $derived(
    (areaHovered || $historyPeekRequested || $demoChipsPinned) &&
      activeToasts.length === 0 &&
      chips.length > 0
  );

  // Back to the latest run whenever the stack fades out.
  $effect(() => {
    if (!showChips) historyIndex = 0;
  });

  // ...and whenever a new run lands.
  $effect(() => {
    void $lastRuns[0]?.at;
    historyIndex = 0;
  });

  // Notify parent when left chip stack is visible (drives BrandingHeader "HISTORY" label).
  $effect(() => {
    onHistoryHoverChange?.(showChips);
  });

  function clearPeekTimers(): void {
    if (peekInTimer) {
      clearTimeout(peekInTimer);
      peekInTimer = null;
    }
    if (peekOutTimer) {
      clearTimeout(peekOutTimer);
      peekOutTimer = null;
    }
  }

  function markPeekSeen(): void {
    if (peekShown) return;
    peekShown = true;
    markHistoryPeekSeen();
    clearPeekTimers();
    peekActive = false;
  }

  // Hint scheduling: once a run exists and there's no active toast, wait 5s
  // of continuous downtime, then show the hint ~3.5s, then fade out.
  // Any new recording (run cleared) or toast activity cancels the peek.
  $effect(() => {
    const downtime = !!run && activeToasts.length === 0 && chips.length > 0;
    if (!peekShown && downtime && !peekActive && !peekInTimer && !peekOutTimer) {
      peekInTimer = setTimeout(() => {
        peekInTimer = null;
        if (peekShown) return;
        peekActive = true;
        peekOutTimer = setTimeout(() => {
          peekOutTimer = null;
          peekActive = false;
          // The peek has played once - persist it as seen. Without this the
          // $effect re-arms and the chips loop show/fade every ~8s until the
          // user happens to hover the area (markPeekSeen was hover-only).
          markPeekSeen();
        }, HINT_SHOW_MS);
      }, 5000);
    } else if (!downtime && (peekInTimer || peekOutTimer || peekActive)) {
      clearPeekTimers();
      peekActive = false;
    }
  });

  // Hovering Copy Last wants history now: fade out any output toasts still up
  // (they block the chips) instead of waiting for their normal dismiss.
  $effect(() => {
    if (!$historyPeekRequested) return;
    for (const toast of activeToasts) outputToasts.dismiss(toast.id);
  });

  // Seeing the chips via the Copy Last button also counts as finding history.
  $effect(() => {
    if ($historyPeekRequested && chips.length > 0) markPeekSeen();
  });

  function handleAreaEnter(): void {
    areaHovered = true;
    markPeekSeen();
  }

  function handleHintPreview(): void {
    if (hintPreviewTimer) clearTimeout(hintPreviewTimer);
    hintPreview = true;
    hintPreviewTimer = setTimeout(() => {
      hintPreviewTimer = null;
      hintPreview = false;
    }, HINT_SHOW_MS);
  }

  function handleFtueReset(): void {
    resetHistoryPeekSeen();
    peekShown = false;
    clearPeekTimers();
    peekActive = false;
  }

  // Debounce projection clear so rapid chip-to-chip hovers don't flicker
  // the right-side bubble. mouseleave on chip A schedules a clear; if
  // mouseenter on chip B fires within the same microtask, we cancel it
  // and just swap the content in place.
  let projectClearTimer: ReturnType<typeof setTimeout> | null = null;
  function projectOn(chip: Chip): void {
    if (!chip.text) return;
    if (projectClearTimer) {
      clearTimeout(projectClearTimer);
      projectClearTimer = null;
    }
    rightProjection.set({ label: chip.label, text: chip.text });
  }
  function projectOff(): void {
    if (projectClearTimer) clearTimeout(projectClearTimer);
    projectClearTimer = setTimeout(() => {
      projectClearTimer = null;
      rightProjection.set(null);
    }, 40);
  }
  async function copyChip(chip: Chip): Promise<void> {
    if (!chip.text) return;
    copiedChipKey = chip.key;
    await rowCopyFeedback.copy(chip.text);
    setTimeout(() => {
      if (copiedChipKey === chip.key) copiedChipKey = null;
    }, 1300);
  }

  let folderErrors: Record<string, boolean> = $state({});

  onMount(() => {
    if (typeof window !== 'undefined') {
      window.addEventListener('ftue-reset', handleFtueReset);
      window.addEventListener('history-peek-reset', handleFtueReset);
      window.addEventListener('history-hint-preview', handleHintPreview);
    }
  });

  function handleToastHover(event: CustomEvent<{ id: string; hovered: boolean }>) {
    outputToasts.setHovered(event.detail.id, event.detail.hovered);
    if (!event.detail.hovered) {
      scheduleToastDismiss(event.detail.id);
    }
  }

  function handleExpand(event: CustomEvent<{ toast: OutputToast }>) {
    const toast = event.detail.toast;
    if (toast.mode === 'save_as_file' && toast.filePath) {
      openFolder(toast.filePath, toast.id);
    }
    // Text popover removed - the last-run chip stack + right-side projection
    // cover the "see full output" need without positioning issues.
  }

  async function openFolder(filePath: string, toastId?: string) {
    try {
      const lastSlash = Math.max(filePath.lastIndexOf('/'), filePath.lastIndexOf('\\'));
      const directory = lastSlash > 0 ? filePath.slice(0, lastSlash) : filePath;
      await mustInvoke('open_directory', { path: directory });
    } catch (error) {
      console.error('Failed to open folder:', error);
      if (toastId) {
        folderErrors = { ...folderErrors, [toastId]: true };
        setTimeout(() => {
          const { [toastId]: _, ...rest } = folderErrors;
          folderErrors = rest;
        }, 2000);
      }
    }
  }

  onDestroy(() => {
    rowCopyFeedback.cleanup();
    if (projectClearTimer) clearTimeout(projectClearTimer);
    rightProjection.set(null);
    clearPeekTimers();
    if (typeof window !== 'undefined') {
      window.removeEventListener('ftue-reset', handleFtueReset);
      window.removeEventListener('history-peek-reset', handleFtueReset);
      window.removeEventListener('history-hint-preview', handleHintPreview);
    }
    if (hintPreviewTimer) clearTimeout(hintPreviewTimer);
  });
</script>

<!-- Hover area on the left side of the button - mirrors TranscriptionPreviewArea width/offset. -->
<div
  class="absolute right-full mr-2 top-0 w-[calc(50vw-8rem)] h-64 z-40"
  onmouseenter={handleAreaEnter}
  onmouseleave={() => (areaHovered = false)}
  role="region"
  aria-label="Output notifications area"
  aria-live="polite"
>
  <!-- Once-per-session hint: gold chevron opening toward the record button.
       Pure decoration (pointer-events: none), so the area underneath still
       gets the hover that dismisses it. -->
  {#if chips.length > 0 || hintPreview}
    <div
      class="history-indicator"
      class:visible={(peekActive || hintPreview) && !areaHovered && !showChips}
      aria-hidden="true"
    >
      <span class="history-indicator-label">{t('lastRun.historyHint')}</span>
      <svg class="history-indicator-chevron" viewBox="0 0 48 152" fill="none">
        <defs>
          <linearGradient
            id="history-chevron-fade"
            x1="6"
            y1="0"
            x2="46"
            y2="0"
            gradientUnits="userSpaceOnUse"
          >
            <stop offset="0" stop-color="rgb(252 211 77)" stop-opacity="0.95" />
            <stop offset="0.55" stop-color="rgb(245 158 11)" stop-opacity="0.45" />
            <stop offset="1" stop-color="rgb(245 158 11)" stop-opacity="0" />
          </linearGradient>
        </defs>
        <path
          d="M46 6 L6 76 L46 146"
          stroke="url(#history-chevron-fade)"
          stroke-width="6"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </div>
  {/if}

  <!-- Content anchored to the right edge (closest to the button) -->
  <div class="absolute right-0 top-0 flex flex-col gap-2 items-end w-full">
    {#each activeToasts as toast (toast.id)}
      <div class="relative animate-fade-in" out:fade={{ duration: 150 }}>
        <OutputToastItem
          {toast}
          halfOpacity={false}
          on:hover={handleToastHover}
          on:expand={handleExpand}
        />
        {#if folderErrors[toast.id]}
          <div class="folder-error">{t('errors.failedToOpenFolder')}</div>
        {/if}
      </div>
    {/each}

    <!-- Last-run action chip stack. Always mounted once chips exist so hover
         in/out is pure CSS (GPU-accelerated opacity). The `peek` class swaps
         to a slower curve only for the one-shot FTUE auto-peek. -->
    {#if chips.length > 0}
      <div
        class="chips-wrap flex flex-col gap-1.5 items-end w-full"
        class:visible={showChips}
        inert={!showChips || undefined}
      >
        {#if showPager || speedLabel}
          <div class="flex items-center gap-2">
            {#if speedLabel}
              <span class="run-speed">
                <Icon name="timer" class="w-3 h-3" strokeWidth={1.8} />
                {speedLabel}
              </span>
            {/if}
            {#if showPager}
              <div class="relative">
                {#if pagerCaptionKey}
                  <!-- On the same line as the header's "HISTORY" label -->
                  <span class="pager-caption">{t(`lastRun.${pagerCaptionKey}`)}</span>
                {/if}
                <div
                  class="flex items-center rounded-lg border border-amber-700/20 bg-amber-950/30 backdrop-blur-sm text-sm font-light"
                >
                  <button
                    type="button"
                    class="pager-btn px-2 py-1.5 rounded-l-lg text-amber-400/70"
                    onclick={() => stepPager(-1)}
                    onmouseenter={() => (pagerHover = 'newer')}
                    onmouseleave={() => (pagerHover = null)}
                    onfocus={() => (pagerHover = 'newer')}
                    onblur={() => (pagerHover = null)}
                    disabled={!canNewer}
                    aria-label={t('lastRun.newer')}
                  >
                    <Icon name="chevron-left" class="w-3.5 h-3.5" strokeWidth={1.8} />
                  </button>
                  <span class="text-amber-200/85 whitespace-nowrap tabular-nums px-1">
                    {historyLabel(historyIndex, $lastRuns.length)}
                  </span>
                  <button
                    type="button"
                    class="pager-btn px-2 py-1.5 rounded-r-lg text-amber-400/70"
                    onclick={() => stepPager(1)}
                    onmouseenter={() => (pagerHover = 'older')}
                    onmouseleave={() => (pagerHover = null)}
                    onfocus={() => (pagerHover = 'older')}
                    onblur={() => (pagerHover = null)}
                    disabled={!canOlder}
                    aria-label={t('lastRun.older')}
                  >
                    <Icon name="chevron-right" class="w-3.5 h-3.5" strokeWidth={1.8} />
                  </button>
                </div>
              </div>
            {/if}
          </div>
        {/if}
        {#each chips as chip (chip.key)}
          {@const justCopied = copiedChipKey === chip.key}
          <button
            type="button"
            class="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-amber-700/20 bg-amber-950/30 backdrop-blur-sm text-sm font-light transition-colors duration-75
              {chip.clickable
              ? 'hover:bg-amber-900/30 cursor-pointer'
              : 'cursor-default opacity-80'}
              {justCopied ? 'bg-amber-900/50 border-amber-500/40' : ''}"
            onmouseenter={() => projectOn(chip)}
            onmouseleave={projectOff}
            onfocus={() => projectOn(chip)}
            onblur={projectOff}
            onclick={() => copyChip(chip)}
            onkeydown={e => (e.key === 'Enter' || e.key === ' ') && copyChip(chip)}
            disabled={!chip.clickable}
            aria-label={chip.clickable ? `Copy ${chip.label}` : chip.label}
          >
            <span class="flex-shrink-0 text-amber-400/70">
              {#if justCopied}
                <Icon name="check" class="w-3.5 h-3.5" strokeWidth={2.5} />
              {:else}
                <Icon name={chip.icon} class="w-3.5 h-3.5" strokeWidth={1.8} />
              {/if}
            </span>
            <span class="text-amber-200/85 whitespace-nowrap">
              {justCopied ? t('common.copied') : chip.label}
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  @keyframes fade-in {
    from {
      opacity: 0;
      transform: translateX(-8px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }

  .animate-fade-in {
    animation: fade-in 0.2s ease-out;
  }

  /* CSS-driven hover transition - GPU-accelerated opacity + translate,
     no JS transition jank, no mount/unmount cost.
     `visibility: hidden` after fade-out so the (expensive) backdrop-blur
     stops painting when the stack isn't visible.
     Hover: 160ms snappy. FTUE peek: 900ms soft. */
  .chips-wrap {
    opacity: 0;
    transform: translate3d(6px, 0, 0);
    visibility: hidden;
    pointer-events: none;
    will-change: opacity, transform;
    transition:
      opacity 160ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 160ms cubic-bezier(0.4, 0, 0.2, 1),
      visibility 0s linear 160ms;
  }

  .chips-wrap.visible {
    opacity: 1;
    transform: translate3d(0, 0, 0);
    visibility: visible;
    pointer-events: auto;
    transition:
      opacity 160ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 160ms cubic-bezier(0.4, 0, 0.2, 1),
      visibility 0s linear 0s;
  }

  /* Vertically centered on the 128px record button (the area is top-aligned with it). */
  .history-indicator {
    position: absolute;
    right: 0;
    top: 64px;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    pointer-events: none;
    opacity: 0;
    visibility: hidden;
    transform: translate3d(8px, -50%, 0);
    transition:
      opacity 900ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 900ms cubic-bezier(0.4, 0, 0.2, 1),
      visibility 0s linear 900ms;
  }

  .history-indicator.visible {
    opacity: 1;
    visibility: visible;
    transform: translate3d(0, -50%, 0);
    transition:
      opacity 900ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 900ms cubic-bezier(0.4, 0, 0.2, 1),
      visibility 0s linear 0s;
  }

  .history-indicator-label {
    font-size: 0.8125rem;
    font-weight: 500;
    letter-spacing: 0.02em;
    white-space: nowrap;
    background: linear-gradient(90deg, rgb(251 191 36 / 0.7), rgb(253 230 138));
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }

  .history-indicator-chevron {
    width: 48px;
    height: 152px;
    flex-shrink: 0;
    overflow: visible;
    filter: drop-shadow(0 0 6px rgb(251 191 36 / 0.35));
  }

  /* A slow leftward nudge toward the label while the hint is up. */
  .history-indicator.visible .history-indicator-chevron {
    animation: chevron-nudge 1.8s ease-in-out infinite;
  }

  @keyframes chevron-nudge {
    0%,
    100% {
      transform: translateX(0);
    }
    50% {
      transform: translateX(-4px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .history-indicator.visible .history-indicator-chevron {
      animation: none;
    }
  }

  .pager-btn {
    transition: background-color 75ms;
    cursor: pointer;
  }

  .pager-btn:hover:not(:disabled) {
    background-color: rgb(120 53 15 / 0.3); /* amber-900/30, matches chip hover */
  }

  .pager-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  /* Speed of the shown run, left of the pager (dim, like the captions). */
  .run-speed {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.6875rem;
    color: rgb(251 191 36 / 0.6);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  /* Latest / Oldest / Older / Newer, lifted onto the header's "HISTORY" line and
     styled like it. The stack is top-aligned with the record button and the
     HISTORY label sits ~48px above the pager's center, hence the offset. */
  .pager-caption {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 28px);
    transform: translateX(-50%);
    font-size: 0.75rem;
    font-weight: 500;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: rgb(251 191 36 / 0.7);
    white-space: nowrap;
    pointer-events: none;
  }

  .folder-error {
    font-size: 0.625rem;
    color: #f87171;
    text-align: right;
    padding: 0.125rem 0.5rem;
    animation: fade-in 0.15s ease-out;
  }
</style>
