<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import type { Readable } from 'svelte/store';

  /** The text to display */
  export let text: Readable<string>;
  /** Whether the preview is visible */
  export let isVisible: Readable<boolean>;
  /** Whether the preview is fading */
  export let isFading: Readable<boolean>;
  /** Current page index for animation keying */
  export let pageIndex: Readable<number>;

  // Reactive word splitting for animation
  $: words = $text
    .trim()
    .split(' ')
    .filter(w => w);
</script>

{#if $isVisible && $text}
  <div
    class="absolute left-full ml-6 top-0 min-w-48 max-w-[calc(50vw-6rem)] max-h-32 overflow-hidden transition-all duration-300 ease-out {$isFading
      ? 'opacity-0 scale-95'
      : 'opacity-100 scale-100'}"
  >
    <div
      class="bg-zinc-800/70 backdrop-blur-sm px-4 py-3 rounded-lg border border-zinc-700/40 shadow-lg"
    >
      <p class="text-zinc-300/90 text-sm font-light leading-relaxed text-left">
        {#each words as word, i ($pageIndex + '-' + i)}
          <span class="inline-block animate-word-fade-in" style="animation-delay: {i * 30}ms"
            >{word}</span
          >{' '}
        {/each}
      </p>
    </div>
  </div>
{/if}
