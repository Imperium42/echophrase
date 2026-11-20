<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Top-left "go back one level" button - mirrors ModalCloseButton's
   * position/style on the opposite corner. Only meant to render when the
   * caller has somewhere to go back to (eg canGoBack derived) - unlike
   * ModalCloseButton, always-visible-and-just-does-nothing isn't the
   * pattern here, so there is no internal disabled state.
   */
  export let onClick: () => void;
  /** Full "Back to X" label - shown as both the tooltip and aria-label so
   * hovering (or a screen reader) tells you exactly where it goes. */
  export let label: string;
</script>

<button
  on:click={onClick}
  class="absolute top-3 left-3 z-20 w-8 h-8 flex items-center justify-center rounded-lg transition-all ease-out duration-200 hover:bg-zinc-500/15 hover:scale-105 active:brightness-90 text-zinc-400 hover:text-zinc-200 cursor-pointer"
  aria-label={label}
  title={label}
  type="button"
>
  <svg width="20" height="20" viewBox="0 0 20 20" fill="none" stroke="currentColor">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 4L6 10L12 16" />
  </svg>
</button>
