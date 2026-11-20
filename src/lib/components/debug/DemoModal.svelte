<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Minimal options modal for the scripted marketing demo (DebugPanel > Demo).
   * Dev-only: only reachable from the dev-only DebugPanel.
   */
  import { demoModalOpen } from '$lib/stores/demo';
  import {
    loadDemoOptions,
    startDemoWithReload,
    type DemoOptions,
  } from '$lib/services/demoService';

  let options: DemoOptions = $state(loadDemoOptions());

  // Re-read the remembered options every time the modal opens.
  $effect(() => {
    if ($demoModalOpen) options = loadDemoOptions();
  });

  const fields: { key: keyof DemoOptions; label: string; hint: string }[] = [
    { key: 'startDelaySec', label: 'Start time', hint: 'countdown after reload' },
    { key: 'moveDelaySec', label: 'Mouse grace', hint: 'pause before the cursor moves' },
    { key: 'recordSec', label: 'Record time', hint: 'how long the fake recording runs' },
    {
      key: 'resetGraceSec',
      label: 'Reset grace',
      hint: 'before GPU/model mock resets',
    },
  ];

  function close(): void {
    demoModalOpen.set(false);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') close();
  }
</script>

<svelte:window onkeydown={$demoModalOpen ? onKeydown : undefined} />

{#if $demoModalOpen}
  <div
    class="fixed inset-0 z-[100] flex items-center justify-center bg-black/60"
    role="presentation"
    onclick={close}
  >
    <div
      class="w-80 rounded-xl border border-zinc-600 bg-zinc-900 p-5 shadow-2xl"
      role="dialog"
      aria-modal="true"
      aria-label="Demo options"
      tabindex="-1"
      onclick={e => e.stopPropagation()}
      onkeydown={e => e.stopPropagation()}
    >
      <h2 class="mb-4 text-base font-semibold text-zinc-100">Demo</h2>
      <form
        class="flex flex-col gap-4"
        onsubmit={e => {
          e.preventDefault();
          startDemoWithReload(options);
        }}
      >
        {#each fields as f (f.key)}
          <label class="flex items-center justify-between gap-3 text-sm text-zinc-100">
            <span>
              {f.label}
              <span class="block text-xs text-zinc-400">{f.hint}</span>
            </span>
            <span class="flex items-center gap-1">
              <input
                type="number"
                min="0"
                max="600"
                step="1"
                bind:value={options[f.key]}
                class="w-16 rounded border border-zinc-500 bg-zinc-800 px-2 py-1.5 text-right text-base text-white tabular-nums [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
              />
              <span class="text-zinc-300">s</span>
            </span>
          </label>
        {/each}
        <label class="flex flex-col gap-1.5 text-sm text-zinc-100">
          <span>
            Transcription result
            <span class="block text-xs text-zinc-400">canned text shown as the result</span>
          </span>
          <input
            type="text"
            maxlength="500"
            bind:value={options.transcriptionText}
            class="w-full rounded border border-zinc-500 bg-zinc-800 px-2 py-1.5 text-sm text-white"
          />
        </label>
        <div class="mt-1 flex justify-end gap-2">
          <button
            type="button"
            class="cursor-pointer rounded px-3 py-1.5 text-sm text-zinc-300 hover:text-white"
            onclick={close}>Cancel</button
          >
          <button
            type="submit"
            class="cursor-pointer rounded bg-fuchsia-600 px-4 py-1.5 text-sm font-medium text-white hover:bg-fuchsia-500"
            >Start</button
          >
        </div>
      </form>
    </div>
  </div>
{/if}
