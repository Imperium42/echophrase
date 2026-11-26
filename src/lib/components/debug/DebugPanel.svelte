<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { mustInvoke } from '$lib/ipc/invoke';
  import { debugLogsService } from '$lib/services/debugLogsService';
  import { debugHidden } from '$lib/stores/debugVisibility';
  import { demoChromeHidden } from '$lib/stores/demo';
  import { bannersHidden } from '$lib/stores/bannerVisibility';
  import { onboardingStore } from '$lib/stores/onboarding';
  import { modelStore } from '$lib/stores/modelStore';
  import { useKeyedCopyFeedback } from '$lib/composables/useCopyFeedback';
  import { onMount, onDestroy } from 'svelte';
  import { isUnsupportedMac } from '$lib/stores/platformCache';
  import { debugShowMockUpdate, isUpdateAvailable } from '$lib/stores/updater';
  import { requestDemo } from '$lib/services/demoService';
  import { Icon } from '$lib/icons';

  const copyFeedback = useKeyedCopyFeedback({ feedbackDuration: 1500 });
  const copiedKey = copyFeedback.copiedKey;

  // Extract stores from service for $ prefix usage
  const logs = debugLogsService.logs;
  const isRefreshing = debugLogsService.isRefreshing;

  // Debug logs panel state
  let showDebugLogs = false;

  // Debug logs panel resizing state
  let logsPanelWidth = 384;
  let logsPanelHeight = 240;
  let isResizing = false;
  let resizeStartX = 0;
  let resizeStartY = 0;
  let resizeStartWidth = 0;
  let resizeStartHeight = 0;

  // Resize functions
  function startResize(event: MouseEvent) {
    isResizing = true;
    resizeStartX = event.clientX;
    resizeStartY = event.clientY;
    resizeStartWidth = logsPanelWidth;
    resizeStartHeight = logsPanelHeight;

    document.body.style.userSelect = 'none';
    document.body.style.cursor = 'nw-resize';

    document.addEventListener('mousemove', handleResize);
    document.addEventListener('mouseup', stopResize);

    event.preventDefault();
  }

  function handleResize(event: MouseEvent) {
    if (!isResizing) return;

    const deltaX = event.clientX - resizeStartX;
    const deltaY = event.clientY - resizeStartY;

    logsPanelWidth = Math.max(200, resizeStartWidth + deltaX);
    logsPanelHeight = Math.max(150, resizeStartHeight + deltaY);
  }

  function stopResize() {
    isResizing = false;

    document.body.style.userSelect = '';
    document.body.style.cursor = '';

    document.removeEventListener('mousemove', handleResize);
    document.removeEventListener('mouseup', stopResize);
  }

  // Fetch logs on mount
  onMount(async () => {
    await debugLogsService.fetch();
  });

  onDestroy(() => copyFeedback.cleanup());
</script>

{#if import.meta.env.DEV && !$debugHidden && !$demoChromeHidden}
  <div
    class="fixed left-4 z-[10000] {$isUnsupportedMac
      ? 'top-[10.5rem]'
      : $isUpdateAvailable
        ? 'top-14'
        : 'top-3'}"
  >
    <div class="flex flex-wrap gap-2 relative z-10 max-w-[calc(100vw/var(--ui-zoom,1)_-_8rem)]">
      <button
        on:click={() => {
          showDebugLogs = !showDebugLogs;
        }}
        class="bg-zinc-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-zinc-400 hover:text-zinc-300 border border-zinc-700/50 cursor-pointer opacity-50 hover:opacity-100 focus:opacity-100 transition-opacity"
      >
        Debug
      </button>

      {#if showDebugLogs}
        <button
          on:click={() => window.location.reload()}
          class="bg-zinc-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-zinc-400 hover:text-zinc-300 border border-zinc-700/50 cursor-pointer whitespace-nowrap"
          title="Reload webview (F5 / Fn+F5 on Mac)"
          aria-label="Reload webview"
        >
          <Icon name="refresh" size={14} />
        </button>

        <button
          on:click={async () => {
            try {
              await mustInvoke('toggle_devtools');
            } catch (e) {
              console.error('[Debug] Failed to toggle devtools:', e);
            }
          }}
          class="bg-zinc-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-zinc-400 hover:text-zinc-300 border border-zinc-700/50 cursor-pointer whitespace-nowrap"
          title="Ctrl+Shift+I"
        >
          DevTools
        </button>

        <!-- The splash normally only exists between process start and the
             frontend calling show_main_window, which is far too brief to
             inspect. This shows the real splash window for 10s on demand. -->
        <button
          on:click={async () => {
            try {
              await mustInvoke('debug_show_splash');
            } catch (e) {
              console.error('[Debug] Failed to show splash:', e);
            }
          }}
          class="bg-zinc-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-zinc-400 hover:text-zinc-300 border border-zinc-700/50 cursor-pointer whitespace-nowrap"
          title="Show the splash window for 10 seconds"
        >
          Splash
        </button>

        <!-- Unload + reload the transcription model to profile cold-load time.
             Fires the normal model-preload-started/finished events so the
             spinner and timing log work exactly like first-boot. -->
        <button
          on:click={async () => {
            try {
              // reloadModel() (unload_model -> preload), NOT clearCache():
              // clearCache only drops the Candle cache, so it never evicted a
              // resident Qwen3/ONNX model and never cleared Rust's LOADED_MODEL
              // record - leaving this button a no-op that reported "0ms cached"
              // instead of the cold-load profile it advertises.
              await modelStore.reloadModel();
            } catch (e) {
              console.error('[Debug] Reload model failed:', e);
            }
          }}
          class="bg-indigo-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-indigo-300 hover:text-indigo-200 border border-indigo-700/50 cursor-pointer whitespace-nowrap"
          title="Unload then preload the model - watch console for timing (cold-start profile)"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="refresh" size={12} />Reload model</span
          >
        </button>

        <!-- Resets all FTUE state (onboarding + history peek). Calls onboardingStore.reset()
             which clears FTUE_KEYS and dispatches 'ftue-reset' event.
             If you add new FTUE localStorage keys, update FTUE_KEYS in onboarding.ts. -->
        <button
          on:click={() => onboardingStore.reset()}
          class="bg-purple-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-purple-400 hover:text-purple-300 border border-purple-700/50 cursor-pointer whitespace-nowrap"
          title="Reset and show welcome screen"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="celebrate" size={12} />Welcome</span
          >
        </button>

        <!-- Scripted marketing demo: reloads the window, then 10s later a fake cursor clicks Record, 2s later Stop,
             then a canned transcription appears. Click again while running to cancel.
             Never touches the real recorder/backend. See services/demoService.ts. -->
        <button
          on:click={requestDemo}
          class="bg-fuchsia-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-fuchsia-400 hover:text-fuchsia-300 border border-fuchsia-700/50 cursor-pointer whitespace-nowrap"
          title="Scripted record demo: opens options, reloads, then runs (click again while running to cancel)"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="mouse-click" size={12} />Demo</span
          >
        </button>

        <!-- Resets ONLY this session's history hint flag, so the gold "Hover for
             history" chevron plays again 5s after the next run's toast is gone.
             Grouped with Welcome: both are FTUE resets. -->
        <button
          on:click={() => {
            window.dispatchEvent(new CustomEvent('history-peek-reset'));
          }}
          class="bg-purple-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-purple-400 hover:text-purple-300 border border-purple-700/50 cursor-pointer whitespace-nowrap"
          title="Reset this session's history hint so it replays after the next run"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="book" size={12} />History FTUE</span
          >
        </button>

        <!-- Plays the history hint right now (no run needed, doesn't mark it seen). -->
        <button
          on:click={() => {
            // Collapse the fan-out so it doesn't cover the hint while it plays.
            showDebugLogs = false;
            window.dispatchEvent(new CustomEvent('history-hint-preview'));
          }}
          class="bg-purple-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-purple-400 hover:text-purple-300 border border-purple-700/50 cursor-pointer whitespace-nowrap"
          title="Show the history hint now (preview only, doesn't count as seen)"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="chevron-left" size={12} />History Hint</span
          >
        </button>

        <button
          on:click={() => debugShowMockUpdate()}
          class="bg-sky-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-sky-300 hover:text-sky-200 border border-sky-700/50 cursor-pointer whitespace-nowrap"
          title="Show update-available modal with mock release notes"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="update-available" size={12} />Updater</span
          >
        </button>

        <!-- Capture group: temporarily clear the screen for a stream or promo
             video. Both windows self-expire after 5m; F4 cancels either one. -->
        <button
          on:click={() => debugHidden.hideFor5Minutes()}
          class="bg-zinc-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-zinc-400 hover:text-zinc-300 border border-zinc-700/50 cursor-pointer whitespace-nowrap"
          title="Hide debug button/settings for 5m; F4 to cancel"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="eye-off" size={12} />Hide 5m</span
          >
        </button>

        <button
          on:click={() => bannersHidden.hideFor5Minutes()}
          class="bg-zinc-800/80 backdrop-blur-sm px-2 py-1 rounded text-xs text-zinc-400 hover:text-zinc-300 border border-zinc-700/50 cursor-pointer whitespace-nowrap"
          title="Hide the top-left update banner for 5m; F4 to cancel"
        >
          <span class="inline-flex items-center gap-1"
            ><Icon name="eye-off" size={12} />Hide Banners 5m</span
          >
        </button>
      {/if}
    </div>

    {#if showDebugLogs}
      <div
        class="mt-2 bg-zinc-900/95 backdrop-blur-sm border border-zinc-700/50 rounded-lg overflow-hidden resize-none select-none"
        style="width: {logsPanelWidth}px; height: {logsPanelHeight}px;"
      >
        <div class="flex justify-between items-center p-2 border-b border-zinc-700/50">
          <span class="text-xs text-zinc-400">Backend Logs</span>
          <div class="flex gap-2">
            <button
              on:click={() => debugLogsService.clear()}
              class="text-xs text-zinc-400 hover:text-zinc-300 transition-all px-2 py-0.5 rounded hover:bg-zinc-700/50 active:bg-zinc-700 active:scale-95 cursor-pointer"
            >
              Clear
            </button>
            <button
              on:click={() => copyFeedback.copy('logs', $logs.join('\n'))}
              class="text-xs transition-all px-2 py-0.5 rounded hover:bg-zinc-700/50 active:bg-zinc-700 active:scale-95 cursor-pointer {$copiedKey ===
              'logs:error'
                ? 'text-red-400'
                : 'text-zinc-400 hover:text-zinc-300'}"
              title="Copy all logs"
            >
              {#if $copiedKey === 'logs'}<Icon
                  name="check-circle"
                  size={14}
                  class="text-green-400"
                />{:else if $copiedKey === 'logs:error'}<Icon
                  name="x-circle"
                  size={14}
                  class="text-red-400"
                />{:else}<Icon name="clipboard" size={14} />{/if}
            </button>
            <button
              on:click={async () => {
                console.log('Debug refresh clicked');
                await debugLogsService.fetch();
              }}
              disabled={$isRefreshing}
              class="text-xs text-cyan-400 hover:text-cyan-300 disabled:text-cyan-600 disabled:cursor-not-allowed transition-all px-2 py-0.5 rounded hover:bg-cyan-400/10 active:bg-cyan-400/20 active:scale-95 cursor-pointer"
            >
              {$isRefreshing ? 'Refreshing...' : 'Refresh'}
            </button>
          </div>
        </div>
        <div
          class="p-2 overflow-y-auto text-xs font-mono text-zinc-300 space-y-1"
          style="height: {logsPanelHeight - 40}px;"
        >
          {#each $logs as log, i (i)}
            <div class="text-wrap break-words">{log}</div>
          {/each}
          {#if $logs.length === 0}
            <div class="text-zinc-500">No debug logs yet - click Refresh</div>
          {/if}
        </div>

        <!-- Resize handle in bottom-right corner -->
        <!-- svelte-ignore a11y-no-static-element-interactions -->
        <div
          class="absolute bottom-0 right-0 w-4 h-4 cursor-nw-resize bg-zinc-700/30 hover:bg-zinc-600/50 transition-colors"
          on:mousedown={startResize}
          title="Drag to resize"
        >
          <!-- Resize grip dots -->
          <div class="absolute bottom-1 right-1 w-2 h-2">
            <div class="w-0.5 h-0.5 bg-zinc-500 absolute bottom-0 right-0"></div>
            <div class="w-0.5 h-0.5 bg-zinc-500 absolute bottom-0 right-1"></div>
            <div class="w-0.5 h-0.5 bg-zinc-500 absolute bottom-1 right-0"></div>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/if}
