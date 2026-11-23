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
  import { mustInvoke } from '$lib/ipc/invoke';
  import { outputToasts, scheduleToastDismiss, type OutputToast } from '$lib/stores/outputToasts';
  import { lastRun } from '$lib/stores/lastRun';
  import { demoChipsPinned } from '$lib/stores/demo';
  import { rightProjection } from '$lib/stores/rightProjection';
  import { useCopyFeedback } from '$lib/composables/useCopyFeedback';
  import { t } from '$lib/locales';
  import { Icon, type IconName } from '$lib/icons';
  import OutputToastItem from './OutputToastItem.svelte';

  /** FTUE peek: after the very first successful run, briefly fade in the
   * history chip stack + "HISTORY" label once after 5s of downtime, then
   * fade out. Actually hovering left dismisses it forever. */
  const PEEK_SHOWN_KEY = 'echophrase_history_peek_shown';
  let peekShown: boolean =
    typeof window !== 'undefined' && localStorage.getItem(PEEK_SHOWN_KEY) === 'true';
  let peekActive = $state(false);
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

  let run = $derived($lastRun);

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
    if (run.speed) {
      const s = run.speed;
      const speedLabel =
        s.transformSeconds !== null
          ? `${formatSeconds(s.transcriptionSeconds)} · ${formatSeconds(s.transformSeconds)} · ${formatSeconds(s.totalSeconds)}`
          : formatSeconds(s.transcriptionSeconds);
      out.push({
        key: 'speed',
        label: speedLabel,
        icon: 'timer',
        text: undefined,
        clickable: false,
      });
    }
    return out;
  });

  /** Seconds with 1 decimal; sub-10ms values (only the scripted demo) render as ms. */
  function formatSeconds(seconds: number): string {
    return seconds < 0.01 ? `${(seconds * 1000).toFixed(1)}ms` : `${seconds.toFixed(1)}s`;
  }

  let showChips = $derived(
    (areaHovered || peekActive || $demoChipsPinned) && activeToasts.length === 0 && chips.length > 0
  );

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
    try {
      localStorage.setItem(PEEK_SHOWN_KEY, 'true');
    } catch {}
    clearPeekTimers();
    peekActive = false;
  }

  // FTUE peek scheduling: once a run exists and there's no active toast, wait
  // 5s of continuous downtime, then force-show chips ~3.5s, then fade out.
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
        }, 3500);
      }, 5000);
    } else if (!downtime && (peekInTimer || peekOutTimer || peekActive)) {
      clearPeekTimers();
      peekActive = false;
    }
  });

  function handleAreaEnter(): void {
    areaHovered = true;
    markPeekSeen();
  }

  function handleFtueReset(): void {
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
    }
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
  <!-- Content anchored to the right edge (closest to the button) -->
  <div class="absolute right-0 top-0 flex flex-col gap-2 items-end w-full">
    {#each activeToasts as toast (toast.id)}
      <div class="relative animate-fade-in">
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
        class:peek={peekActive && !areaHovered}
        inert={!showChips || undefined}
      >
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

  .chips-wrap.peek {
    transition:
      opacity 900ms cubic-bezier(0.4, 0, 0.2, 1),
      transform 900ms cubic-bezier(0.4, 0, 0.2, 1),
      visibility 0s linear 0s;
  }

  .folder-error {
    font-size: 0.625rem;
    color: #f87171;
    text-align: right;
    padding: 0.125rem 0.5rem;
    animation: fade-in 0.15s ease-out;
  }
</style>
