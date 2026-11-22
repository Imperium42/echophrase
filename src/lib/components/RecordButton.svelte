<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { onDestroy } from 'svelte';
  import { classifyRecordingError } from '$lib/types/errors';
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';

  interface Props {
    isRecording?: boolean;
    isProcessing?: boolean;
    isModelLoading?: boolean;
    toggleRecording: () => void;
    errorMessage?: string;
    /** Accepted for parent prop pass-through; the component renders its own time elsewhere. */
    formatTime?: (seconds: number) => string;
    serverStatus?: 'connecting' | 'ready' | 'error';
    showIntroPulse?: boolean;
    recordingMode?: 'push_to_talk' | 'open_mic';
    isOpenMicListening?: boolean;
    isOpenMicSilenceCounting?: boolean;
    /** Silence timeout in seconds (from settings, default 5). */
    silenceTimeout?: number;
    /** Recording elapsed time in whole seconds - used to sync the pulse animation. */
    recordingTime?: number;
    /** Action callbacks for structured error recovery. */
    errorActions?: {
      openDownloadModal?: () => void;
      openGpuSettings?: () => void;
      openAudioSettings?: () => void;
      retry?: () => void;
    };
  }

  let {
    isRecording = false,
    isProcessing = false,
    isModelLoading = false,
    toggleRecording,
    errorMessage = '',
    formatTime: _formatTime,
    serverStatus = 'ready',
    showIntroPulse = false,
    recordingMode = 'push_to_talk',
    isOpenMicListening = false,
    isOpenMicSilenceCounting = false,
    silenceTimeout = 5,
    recordingTime = 0,
    errorActions = {},
  }: Props = $props();

  // Countdown timer for silence state. The Rust backend fires
  // silence_started at half the timeout, so we count down from
  // ceil(timeout / 2).
  let silenceCountdown = $state(0);
  let countdownInterval: ReturnType<typeof setInterval> | null = null;

  function startCountdown() {
    stopCountdown();
    silenceCountdown = Math.ceil(silenceTimeout / 2);
    countdownInterval = setInterval(() => {
      silenceCountdown -= 1;
      if (silenceCountdown <= 0) stopCountdown();
    }, 1000);
  }

  function stopCountdown() {
    if (countdownInterval) {
      clearInterval(countdownInterval);
      countdownInterval = null;
    }
    silenceCountdown = 0;
  }

  // Drive the countdown lifecycle off the silence-counting prop edges.
  // $effect handles the prev-value tracking that the legacy $: block did
  // by hand (no manual prevSilenceCounting flag needed - the closure
  // captures the previous value naturally).
  $effect(() => {
    if (isOpenMicSilenceCounting) {
      startCountdown();
    } else {
      stopCountdown();
    }
  });

  onDestroy(stopCountdown);

  // Pulse sync: driven by the JS countup tick, not a free-running CSS timer.
  // Every 2 seconds (when recordingTime is a positive even number) we briefly
  // clear the active class then restore it on the next microtask - this forces
  // the browser to restart the one-shot CSS animation from frame 0, keeping
  // the pulse peak in lockstep with the moment the displayed counter changes.
  let pulseSyncActive = $state(false);

  $effect(() => {
    // Access recordingTime inside the effect body so Svelte tracks it.
    const t = recordingTime;
    if (!isRecording || t <= 0 || t % 2 !== 0) return;
    // Retrigger: toggle off -> microtask -> toggle on.
    pulseSyncActive = false;
    Promise.resolve().then(() => {
      pulseSyncActive = true;
    });
  });

  // When recording starts, fire the first pulse immediately (t=0 is excluded
  // above to avoid double-trigger on mount; this covers the initial activation).
  $effect(() => {
    if (isRecording) {
      pulseSyncActive = true;
    } else {
      pulseSyncActive = false;
    }
  });

  const isOpenMic = $derived(recordingMode === 'open_mic');

  const rimClass = $derived(
    isProcessing && isOpenMic
      ? 'rim-cyan'
      : isRecording
        ? isOpenMic
          ? isOpenMicListening
            ? 'rim-violet'
            : isOpenMicSilenceCounting
              ? 'rim-red-soft'
              : 'rim-red'
          : 'rim-red'
        : 'rim-teal'
  );

  const isDisabled = $derived(isProcessing || isModelLoading || serverStatus === 'connecting');

  // Classify raw error into structured error with action
  const structuredError = $derived(
    errorMessage ? classifyRecordingError(errorMessage, errorActions) : null
  );

  let copied = $state(false);
  function copyError() {
    if (errorMessage) {
      navigator.clipboard.writeText(errorMessage);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    }
  }
</script>

<!-- Glass Record Button - Spotify Style -->
<div class="relative mb-8 record-button-container">
  <button
    onclick={toggleRecording}
    disabled={isDisabled}
    class="record-btn group relative w-32 h-32 rounded-full flex items-center justify-center focus:outline-none cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed bg-zinc-500/5 backdrop-blur-md transition-[transform,background-color,box-shadow] duration-200 ease-out {rimClass} {isRecording ||
    isProcessing
      ? ''
      : 'hover:bg-zinc-500/20 hover:scale-105 active:brightness-95'}"
    aria-label={isModelLoading
      ? t('recording.ariaLoadingModel')
      : serverStatus === 'connecting'
        ? t('recording.ariaServerStarting')
        : isProcessing
          ? t('recording.ariaProcessing')
          : isRecording
            ? t('recording.ariaStopRecording')
            : t('recording.ariaStartRecording')}
  >
    <!-- All states always in DOM - CSS controls visibility to prevent layout shifts -->

    <!-- Model Loading Animation -->
    <div class="icon-state flex items-center justify-center" class:icon-visible={isModelLoading}>
      <div
        class="w-8 h-8 border-2 border-zinc-400/30 border-t-purple-400 rounded-full"
        class:animate-spin={isModelLoading}
      ></div>
    </div>

    <!-- Server Connecting Animation -->
    <div
      class="icon-state flex items-center justify-center"
      class:icon-visible={!isModelLoading && serverStatus === 'connecting'}
    >
      <div
        class="w-8 h-8 border-2 border-zinc-400/30 border-t-cyan-400 rounded-full"
        class:animate-spin={!isModelLoading && serverStatus === 'connecting'}
      ></div>
    </div>

    <!-- Processing Animation (wave bars always in DOM) -->
    <div
      class="icon-state flex items-center gap-1"
      class:icon-visible={!isModelLoading && serverStatus !== 'connecting' && isProcessing}
    >
      <div
        class="w-2 h-8 bg-gradient-to-t from-cyan-400 to-purple-500 rounded-full wave-bar"
        class:animate-wave={isProcessing}
        style="animation-delay: 0s"
      ></div>
      <div
        class="w-2 h-8 bg-gradient-to-t from-cyan-400 to-purple-500 rounded-full wave-bar"
        class:animate-wave={isProcessing}
        style="animation-delay: 0.2s"
      ></div>
      <div
        class="w-2 h-8 bg-gradient-to-t from-cyan-400 to-purple-500 rounded-full wave-bar"
        class:animate-wave={isProcessing}
        style="animation-delay: 0.4s"
      ></div>
    </div>

    <!-- Stop/Recording Icon - mode-aware -->
    <div
      class="icon-state"
      class:icon-visible={!isModelLoading &&
        serverStatus !== 'connecting' &&
        !isProcessing &&
        isRecording}
    >
      {#if isOpenMic}
        <Icon
          name="mic"
          class="w-12 h-12 transition-colors duration-200 {isOpenMicListening
            ? 'text-violet-400'
            : isOpenMicSilenceCounting
              ? 'text-red-400/60'
              : 'text-red-400'}"
          size={48}
        />
      {:else}
        <!-- Push-to-talk stop indicator: filled rounded rectangle. Intentionally
             kept as inline SVG - Lucide is stroke-only and we want a solid stop
             square here. -->
        <svg
          class="w-12 h-12 text-red-400"
          fill="currentColor"
          viewBox="0 0 24 24"
          aria-hidden="true"
        >
          <path
            fill-rule="evenodd"
            d="M4.5 7.5a3 3 0 013-3h9a3 3 0 013 3v9a3 3 0 01-3 3h-9a3 3 0 01-3-3v-9z"
            clip-rule="evenodd"
          />
        </svg>
      {/if}
    </div>

    <!-- Idle / default state -->
    <div
      class="icon-state"
      class:icon-visible={!isModelLoading &&
        serverStatus !== 'connecting' &&
        !isProcessing &&
        !isRecording}
    >
      <Icon
        name="mic"
        class="w-12 h-12 text-cyan-400 group-hover:text-cyan-300 transition-colors"
        size={48}
      />
    </div>

    <!-- Sonar pulse (always in DOM to prevent layout shift) -->
    <div
      class="absolute inset-0 rounded-full sonar-ring-chargeup pointer-events-none"
      class:animate-sonar-chargeup={showIntroPulse}
    ></div>
  </button>

  <!-- Recording pulse (sibling of button to avoid contain:layout clipping) -->
  <div
    class="absolute inset-0 rounded-full pointer-events-none {isOpenMic
      ? isOpenMicListening
        ? 'recording-pulse-open-mic'
        : 'recording-pulse-open-mic-capturing'
      : 'recording-pulse'}"
    class:recording-pulse-active={!isOpenMic && pulseSyncActive}
    class:recording-pulse-open-mic-active={false}
    class:recording-pulse-open-mic-capturing-active={isRecording &&
      !isProcessing &&
      isOpenMic &&
      !isOpenMicListening &&
      !isOpenMicSilenceCounting}
  ></div>

  <!-- Open Mic persistent state pill -->
  {#if isOpenMic && (isRecording || isProcessing)}
    <div
      class="absolute top-full mt-5 left-1/2 -translate-x-1/2 flex items-center gap-2 px-3.5 py-1.5 rounded-full whitespace-nowrap pointer-events-none transition-colors duration-200
      {isProcessing
        ? 'bg-cyan-500/10 border border-cyan-400/20'
        : isOpenMicListening
          ? 'bg-violet-500/5 border border-violet-400/10'
          : isOpenMicSilenceCounting
            ? 'bg-red-500/5 border border-red-400/10'
            : 'bg-red-500/10 border border-red-400/20'}"
    >
      {#if isOpenMicSilenceCounting && silenceCountdown > 0}
        <span class="silence-countdown text-xs font-bold text-red-400/60 tabular-nums"
          >{silenceCountdown}</span
        >
      {:else}
        <span class="relative flex h-2 w-2">
          {#if isProcessing}
            <span class="relative inline-flex rounded-full h-2 w-2 bg-cyan-400 animate-pulse"
            ></span>
          {:else if isOpenMicListening}
            <span class="relative inline-flex rounded-full h-2 w-2 status-dot status-dot-listening"
            ></span>
          {:else}
            <span class="relative inline-flex rounded-full h-2 w-2 status-dot status-dot-recording"
            ></span>
          {/if}
        </span>
      {/if}
      <span
        class="text-xs font-medium {isProcessing
          ? 'text-cyan-300'
          : isOpenMicListening
            ? 'text-violet-300/50'
            : isOpenMicSilenceCounting
              ? 'text-red-300/40'
              : 'text-red-300'}"
        role="status"
        aria-live="polite"
      >
        {isProcessing
          ? t('recording.statusTranscribing')
          : isOpenMicListening
            ? t('recording.statusListening')
            : t('recording.statusRecording')}
      </span>
    </div>
  {/if}

  <!-- Error message: below the record button, centered -->
  {#if structuredError}
    <div
      class="absolute top-full mt-4 left-1/2 -translate-x-1/2 w-max max-w-xs sm:max-w-sm pointer-events-auto z-10 transition-all duration-500 ease-out opacity-100 scale-100"
    >
      <div class="bg-red-900/80 px-4 py-2.5 rounded-lg border border-red-700/50 shadow-lg">
        <div class="flex items-center gap-2">
          <p class="text-red-300 text-sm leading-relaxed line-clamp-5 flex-1">
            {structuredError.message}
          </p>
          <div class="flex items-center gap-1 flex-shrink-0">
            {#if structuredError.action}
              <button class="error-action-btn" onclick={structuredError.action.callback}>
                {structuredError.action.label}
              </button>
            {/if}
            {#if structuredError.type === 'generic' && errorMessage.length > 30}
              <button class="error-copy-btn" onclick={copyError} aria-label={t('common.copyError')}>
                {#if copied}
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M5 13l4 4L19 7"
                    />
                  </svg>
                {:else}
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      stroke-width="2"
                      d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                    />
                  </svg>
                {/if}
              </button>
            {/if}
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .record-button-container {
    contain: style;
  }

  /* Record button rim - single box-shadow stack instead of border+ring+shadow.
   * Layered shadows antialias smoothly on rounded-full, unlike border which
   * fights backdrop-filter at the compositor level and produces jaggies.
   *
   * Stack per state:
   *   1. inset stroke - the visible rim color
   *   2. outer halo   - soft color bleed (0-spread, small blur)
   *   3. ambient glow - larger color-tinted drop shadow for depth
   *   4. base shadow  - black shadow for separation from background
   */
  .record-btn {
    /* Force own compositor layer so the rounded clip + backdrop-filter + shadows
     * all composite together at subpixel precision. */
    transform: translateZ(0);
    -webkit-font-smoothing: antialiased;
  }

  .record-btn.rim-teal {
    box-shadow:
      inset 0 0 0 1.5px rgba(45, 212, 191, 0.45),
      0 0 0 1px rgba(45, 212, 191, 0.08),
      0 12px 32px -12px rgba(45, 212, 191, 0.28),
      0 4px 16px rgba(0, 0, 0, 0.35);
  }
  .record-btn.rim-teal:hover {
    box-shadow:
      inset 0 0 0 1.5px rgba(45, 212, 191, 0.7),
      0 0 0 1px rgba(45, 212, 191, 0.15),
      0 18px 44px -12px rgba(45, 212, 191, 0.45),
      0 4px 16px rgba(0, 0, 0, 0.4);
  }
  .record-btn.rim-teal:focus-visible {
    box-shadow:
      inset 0 0 0 1.5px rgba(45, 212, 191, 0.7),
      0 0 0 4px rgba(45, 212, 191, 0.22),
      0 18px 44px -12px rgba(45, 212, 191, 0.45),
      0 4px 16px rgba(0, 0, 0, 0.4);
  }

  .record-btn.rim-red {
    box-shadow:
      inset 0 0 0 1.5px rgba(248, 113, 113, 0.7),
      0 0 0 3px rgba(248, 113, 113, 0.18),
      0 12px 32px -12px rgba(248, 113, 113, 0.35),
      0 4px 16px rgba(0, 0, 0, 0.35);
  }

  .record-btn.rim-red-soft {
    box-shadow:
      inset 0 0 0 1.5px rgba(248, 113, 113, 0.4),
      0 0 0 3px rgba(248, 113, 113, 0.08),
      0 12px 32px -12px rgba(248, 113, 113, 0.2),
      0 4px 16px rgba(0, 0, 0, 0.35);
  }

  .record-btn.rim-violet {
    box-shadow:
      inset 0 0 0 1.5px rgba(167, 139, 250, 0.42),
      0 0 0 3px rgba(167, 139, 250, 0.08),
      0 12px 32px -12px rgba(167, 139, 250, 0.28),
      0 4px 16px rgba(0, 0, 0, 0.35);
  }

  .record-btn.rim-cyan {
    box-shadow:
      inset 0 0 0 1.5px rgba(34, 211, 238, 0.72),
      0 0 0 3px rgba(34, 211, 238, 0.18),
      0 12px 32px -12px rgba(34, 211, 238, 0.35),
      0 4px 16px rgba(0, 0, 0, 0.35);
  }

  /* Icon states - always in DOM, fade in/out */
  .icon-state {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    visibility: hidden;
    transition: opacity 150ms ease-out;
  }

  .icon-visible {
    opacity: 1;
    visibility: visible;
  }

  /* Wave bars - GPU layer always active to prevent jank during transcription */
  .wave-bar {
    transform-origin: center center;
    transform: scaleY(0.625) translateZ(0);
    backface-visibility: hidden;
  }

  /* Button stacking context */
  .record-button-container button {
    contain: layout style;
    isolation: isolate;
  }

  @keyframes wave {
    0%,
    100% {
      transform: scaleY(0.625) translateZ(0);
      opacity: 0.3;
    }
    50% {
      transform: scaleY(1) translateZ(0);
      opacity: 1;
    }
  }

  .animate-wave {
    animation: wave 1.2s ease-in-out infinite;
    will-change: transform, opacity;
    contain: strict;
  }

  /* Chargeup ring - gradient border with glow (hidden by default) */
  .sonar-ring-chargeup {
    background: transparent;
    border: 3px solid transparent;
    background-image:
      linear-gradient(#09090b, #09090b),
      linear-gradient(180deg, rgb(167, 139, 250) 0%, rgb(45, 212, 191) 100%);
    background-origin: border-box;
    background-clip: padding-box, border-box;
    /* Hidden by default - only visible during animation */
    opacity: 0;
    /* Hidden by default - GPU layer created when animation starts */
    will-change: transform, opacity;
  }

  /* Chargeup - small pulse with glow aura */
  @keyframes sonar-chargeup {
    0% {
      transform: scale(1);
      opacity: 0;
      box-shadow:
        0 0 15px 4px rgba(167, 139, 250, 0.5),
        0 0 30px 8px rgba(45, 212, 191, 0.3);
    }
    25% {
      opacity: 0.7;
    }
    99% {
      transform: scale(1.4);
      opacity: 0;
      box-shadow:
        0 0 25px 8px rgba(167, 139, 250, 0.3),
        0 0 50px 15px rgba(45, 212, 191, 0.15);
    }
    100% {
      transform: scale(1.4);
      opacity: 0;
      visibility: hidden;
      box-shadow: none;
    }
  }

  .animate-sonar-chargeup {
    animation: sonar-chargeup 600ms ease-out forwards;
  }

  /* Recording pulse (always in DOM, GPU layer only when active) */
  .recording-pulse {
    background: rgba(248, 113, 113, 0.15);
    opacity: 0;
  }

  .recording-pulse-active {
    will-change: transform, opacity;
    /* One-shot 2s pulse - re-triggered from JS every 2 seconds in sync with the
     * countup tick. forwards keeps the final (opacity:0) frame so the ring does
     * not snap back to visible between triggers. */
    animation: recording-ping 2s cubic-bezier(0, 0, 0.2, 1) forwards;
  }

  @keyframes recording-ping {
    0% {
      transform: scale(1);
      opacity: 0.6;
    }
    75%,
    100% {
      transform: scale(1.5);
      opacity: 0;
    }
  }

  /* Open mic recording pulse - violet instead of red */
  .recording-pulse-open-mic {
    background: rgba(167, 139, 250, 0.15);
    opacity: 0;
  }

  .recording-pulse-open-mic-active {
    will-change: transform, opacity;
    animation: recording-ping-violet 2s cubic-bezier(0, 0, 0.2, 1) infinite;
  }

  @keyframes recording-ping-violet {
    0% {
      transform: scale(1);
      opacity: 0.5;
    }
    75%,
    100% {
      transform: scale(1.5);
      opacity: 0;
    }
  }

  /* Open mic capturing pulse - red when speech detected */
  .recording-pulse-open-mic-capturing {
    background: rgba(248, 113, 113, 0.15);
    opacity: 0;
  }

  .recording-pulse-open-mic-capturing-active {
    will-change: transform, opacity;
    animation: recording-ping-red 1.2s cubic-bezier(0, 0, 0.2, 1) infinite;
  }

  @keyframes recording-ping-red {
    0% {
      transform: scale(1);
      opacity: 0.6;
    }
    75%,
    100% {
      transform: scale(1.5);
      opacity: 0;
    }
  }

  /* Error action button - ghost style matching error color */
  .error-action-btn {
    /* position+z-index establish a stacking context so the button can never be
       eclipsed by a sibling layer (macOS WebKit was eating the hover/click). */
    position: relative;
    z-index: 1;
    pointer-events: auto;
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 0.375rem;
    color: #fca5a5;
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.25rem 0.625rem;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background 0.15s ease,
      border-color 0.15s ease;
  }

  .error-action-btn:hover {
    background: rgba(239, 68, 68, 0.25);
    border-color: rgba(239, 68, 68, 0.5);
  }

  /* Copy error button */
  .error-copy-btn {
    background: none;
    border: none;
    color: #71717a;
    cursor: pointer;
    padding: 0.25rem;
    border-radius: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.15s ease;
  }

  .error-copy-btn:hover {
    color: #a1a1aa;
  }

  /* Silence countdown - replaces the dot, must visually align with "Recording" text.
   * Default vertical centering aligns box centers, but digit glyphs sit on cap-height
   * while lowercase letters sit on x-height, causing visual offset. Nudge down to compensate. */
  .silence-countdown {
    display: inline-block;
    line-height: 1;
    transform: translateY(1px);
  }

  /* Status dots - shared slow pulse animation, color varies by state */
  .status-dot {
    animation: status-pulse 2s ease-in-out infinite;
  }

  .status-dot-listening {
    background: #a78bfa;
    box-shadow:
      0 0 6px rgba(167, 139, 250, 0.8),
      0 0 12px rgba(167, 139, 250, 0.4);
  }

  .status-dot-recording {
    background: #f87171;
    box-shadow:
      0 0 6px rgba(248, 113, 113, 0.8),
      0 0 12px rgba(248, 113, 113, 0.4);
  }

  @keyframes status-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }
</style>
