<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Output Toast Item
   *
   * Active output-confirmation chip. Shares the compact "chip" shape used by the
   * last-run history chip stack, but with a zinc/neutral palette (not amber) to
   * distinguish live confirmations from resting history.
   */
  import { createEventDispatcher } from 'svelte';
  import type { OutputToast } from '$lib/stores/outputToasts';
  import { Icon, type IconName } from '$lib/icons';

  export let toast: OutputToast;
  export let halfOpacity: boolean = false;

  const dispatch = createEventDispatcher<{
    hover: { id: string; hovered: boolean };
    expand: { toast: OutputToast };
  }>();

  let isHovered = false;

  function handleMouseEnter() {
    isHovered = true;
    dispatch('hover', { id: toast.id, hovered: true });
  }

  function handleMouseLeave() {
    isHovered = false;
    dispatch('hover', { id: toast.id, hovered: false });
  }

  function handleClick() {
    dispatch('expand', { toast });
  }

  $: iconName =
    (
      {
        copy_to_clipboard: 'clipboard',
        paste_at_cursor: 'paste',
        save_as_file: 'save',
        transformed: 'sparkles',
      } as Record<string, IconName>
    )[toast.mode] ?? 'check';
</script>

<div
  class="relative group/toast"
  on:mouseenter={handleMouseEnter}
  on:mouseleave={handleMouseLeave}
  role="status"
  aria-live="polite"
>
  <button
    type="button"
    class="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-zinc-700/40 bg-zinc-900/60 backdrop-blur-sm text-sm font-light transition-colors duration-150 cursor-pointer
      {isHovered ? 'bg-zinc-800/80 border-zinc-600/60' : ''}
      {halfOpacity && !isHovered ? 'opacity-50' : 'opacity-100'}"
    on:click={handleClick}
  >
    <span class="flex-shrink-0 text-emerald-400/80">
      <Icon name={iconName} class="w-3.5 h-3.5" strokeWidth={1.8} />
    </span>
    <span class="text-zinc-200/90 whitespace-nowrap flex items-center gap-1">
      {toast.label}
      {#if toast.mode === 'save_as_file'}
        <span
          class="transition-all duration-200 overflow-hidden flex-shrink-0 text-zinc-400
            {isHovered ? 'opacity-100 w-3.5' : 'opacity-0 w-0'}"
        >
          <Icon name="folder" class="w-3.5 h-3.5" strokeWidth={1.8} />
        </span>
      {/if}
    </span>
  </button>
</div>
