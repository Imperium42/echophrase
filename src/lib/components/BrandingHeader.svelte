<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from '$lib/locales';
  import BrandLogo from './BrandLogo.svelte';

  export let isRecording = false;
  export let recordingTime = 0;
  export let formatTime: (seconds: number) => string;
  export let subtitleReady = true; // For staggered animation
  export let isOpenMicActive = false;
  /** Whether the history hover state is active (left or right side) */
  export let isShowingHistory = false;

  // Tagline auto-fade after 5 seconds (on first mount only)
  let showTagline = true;

  onMount(() => {
    const timer = setTimeout(() => {
      showTagline = false;
    }, 5000);

    return () => clearTimeout(timer);
  });

  // Subtitle visible when: subtitleReady AND showTagline AND not recording AND not showing history
  $: subtitleVisible = subtitleReady && showTagline && !isRecording && !isShowingHistory;
</script>

<!-- Minimal Branding -->
<div class="mb-6 text-center">
  <div class="mb-2">
    <BrandLogo />
  </div>

  <!-- Subtitle with smooth transition (fades out after 5s or when recording) -->
  <div class="relative h-6 flex items-center justify-center">
    <p
      class="text-zinc-400 text-sm absolute transition-opacity duration-700 ease-out whitespace-nowrap {subtitleVisible
        ? 'opacity-100'
        : 'opacity-0 pointer-events-none'}"
    >
      {t('branding.tagline')}
    </p>

    <!-- History hover: amber "History" label -->
    <div
      class="absolute transition-opacity duration-300 ease-out {isShowingHistory && !isRecording
        ? 'opacity-100'
        : 'opacity-0 pointer-events-none'}"
    >
      <span class="text-xs font-medium text-amber-400/70 tracking-wider uppercase"
        >{t('transcription.history')}</span
      >
    </div>

    <!-- Push-to-talk: red timer -->
    <div
      class="absolute transition-opacity duration-500 ease-out {isRecording && !isOpenMicActive
        ? 'opacity-100'
        : 'opacity-0 pointer-events-none'}"
    >
      <div
        class="bg-zinc-900/80 backdrop-blur-sm px-5 py-1.5 rounded-full border border-zinc-700/50"
      >
        <span class="text-red-400 font-mono text-sm font-bold">{formatTime(recordingTime)}</span>
      </div>
    </div>

    <!-- Open Mic: badge-style mode indicator -->
    <div
      class="absolute transition-opacity duration-500 ease-out {isRecording && isOpenMicActive
        ? 'opacity-100'
        : 'opacity-0 pointer-events-none'}"
    >
      <div
        class="open-mic-badge inline-flex items-center px-2.5 py-0.5 rounded-md border border-violet-400/40 bg-violet-500/10"
      >
        <span class="text-violet-300 text-[10px] font-bold tracking-[0.12em] uppercase"
          >{t('audioInput.openMic')}</span
        >
      </div>
    </div>
  </div>
</div>

<style>
  /* Open Mic badge - violet accent with subtle glow */
  .open-mic-badge {
    box-shadow:
      0 0 12px rgba(167, 139, 250, 0.15),
      inset 0 0 8px rgba(167, 139, 250, 0.05);
    backdrop-filter: blur(4px);
  }
</style>
