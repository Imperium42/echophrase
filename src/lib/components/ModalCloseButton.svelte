<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { t } from '$lib/locales';

  export let onClose: () => void;

  // ESC key handler
  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeydown);
  });
</script>

<button
  on:click={onClose}
  class="absolute top-3 right-3 z-20 w-8 h-8 flex items-center justify-center rounded-lg transition-all ease-out duration-200 hover:bg-red-500/15 hover:scale-105 active:brightness-90 text-zinc-400 hover:text-red-400 cursor-pointer"
  aria-label={t('common.closeEsc')}
  title={t('common.closeEsc')}
  type="button"
>
  <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor">
    <path
      stroke-linecap="round"
      stroke-linejoin="round"
      stroke-width="1.5"
      d="M6 6L14 14M6 14L14 6"
    />
  </svg>
</button>
