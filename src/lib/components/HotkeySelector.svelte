<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  type Props = {
    displayHotkey?: string;
    startHotkeyRecording: () => void;
    isRecordingHotkey?: boolean;
    recordedKeys?: string[];
    countdown?: number;
  };

  let {
    displayHotkey = '',
    startHotkeyRecording,
    isRecordingHotkey = false,
    recordedKeys = [],
    countdown = 0,
  }: Props = $props();
</script>

<!-- Dynamic Hotkey Display -->
<div class="text-center">
  <button
    onclick={startHotkeyRecording}
    class="group relative transition-all duration-300 ease-out hover:scale-105 focus:outline-none focus:ring-2 focus:ring-cyan-400/50 rounded-lg"
    disabled={isRecordingHotkey}
  >
    <kbd
      class="px-3 py-1.5 bg-zinc-800 border border-zinc-600 rounded-lg font-mono text-sm text-cyan-400 transition-colors {isRecordingHotkey
        ? 'animate-pulse bg-orange-800 border-orange-600 text-orange-300'
        : 'cursor-pointer group-hover:bg-zinc-700 group-hover:border-zinc-500'}"
    >
      {displayHotkey}
    </kbd>

    {#if isRecordingHotkey}
      {#if recordedKeys.length > 0}
        <div
          class="absolute -top-8 left-1/2 transform -translate-x-1/2 text-xs text-orange-400 whitespace-nowrap"
        >
          Saving in {countdown}...
        </div>
      {/if}
      <div
        class="absolute -bottom-6 left-1/2 transform -translate-x-1/2 text-xs text-zinc-500 whitespace-nowrap"
      >
        esc to cancel
      </div>
    {:else}
      <div
        class="absolute left-full ml-2 top-1/2 -translate-y-1/2 text-xs text-zinc-500 opacity-0 group-hover:opacity-100 transition-opacity whitespace-nowrap"
      >
        Change
      </div>
    {/if}
  </button>
</div>
