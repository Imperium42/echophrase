<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { useFadeOnIdle } from '$lib/composables/useFadeOnIdle';
  import { onMount, onDestroy } from 'svelte';
  import { Icon } from '$lib/icons';
  import { t } from '$lib/locales';

  /**
   * Top-right settings gear. Fades after a few idle seconds, like the other
   * chrome around the record button.
   */
  type Props = {
    onClick?: () => void;
  };

  let { onClick = () => {} }: Props = $props();

  const fade = useFadeOnIdle({ delay: 5000 });
  const fading = fade.fading;
  const hovered = fade.hovered;

  onMount(() => fade.startFade());
  onDestroy(() => fade.cleanup());
</script>

<div
  class="relative transition-opacity duration-200 {$fading && !$hovered
    ? 'opacity-40'
    : 'opacity-100'}"
  role="presentation"
  onmouseenter={fade.handleEnter}
  onmouseleave={fade.handleLeave}
>
  <button
    onclick={onClick}
    class="w-10 h-10 flex items-center justify-center rounded-lg transition-colors duration-200 ease-out focus:outline-none focus:ring-2 focus:ring-zinc-600 cursor-pointer bg-zinc-800/50 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
    aria-label={t('settings.title')}
  >
    <Icon name="settings" class="w-6 h-6" strokeWidth={2} />
  </button>
</div>
