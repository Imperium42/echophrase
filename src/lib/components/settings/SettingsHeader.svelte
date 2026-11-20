<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Settings Header
   *
   * Clean, minimal header with simple icon, search input, and close button.
   * Modern flat design - no boxed icon.
   */

  import { Icon } from '$lib/icons';
  import { t } from '$lib/locales';

  interface Props {
    /** Header title */
    title?: string;
    /** Close button callback */
    onClose?: () => void;
    /** Search query (bound from parent) */
    searchQuery?: string;
  }

  let { title = 'Settings', onClose, searchQuery = $bindable('') }: Props = $props();

  let searchInput: HTMLInputElement | null = null;

  /** Focus the search input - called by parent when the modal opens */
  export function focus() {
    searchInput?.focus();
  }

  function handleKeydown(e: KeyboardEvent) {
    // Ctrl/Cmd+F focuses search
    if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
      e.preventDefault();
      searchInput?.focus();
    }
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    if (searchQuery) {
      // Typed something: clear query, keep input focused, swallow event
      e.preventDefault();
      e.stopPropagation();
      searchQuery = '';
    }
    // Empty (no trim - literally nothing typed): let the event propagate so the
    // global modal-controller ESC listener closes the settings modal.
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<header class="settings-header">
  <Icon
    name="settings"
    class="w-5 h-5 shrink-0 text-zinc-600 transition-colors duration-200"
    size={20}
    strokeWidth={1.5}
  />
  <h2 class="header-title">{title}</h2>
  <div class="search-container">
    <Icon name="search" class="w-3.5 h-3.5 shrink-0 text-zinc-600" size={14} strokeWidth={2} />
    <input
      bind:this={searchInput}
      bind:value={searchQuery}
      type="text"
      placeholder={t('settings.searchPlaceholder')}
      class="search-input"
      aria-label={t('settings.searchAriaLabel')}
      onkeydown={handleSearchKeydown}
    />
    {#if searchQuery}
      <button
        class="search-clear"
        onclick={() => (searchQuery = '')}
        aria-label={t('settings.clearSearch')}
      >
        <Icon name="close" size={12} strokeWidth={2} />
      </button>
    {/if}
  </div>
  {#if onClose}
    <button class="close-btn" onclick={onClose} aria-label={t('settings.closeSettings')}>
      <Icon name="close" size={16} strokeWidth={1.5} />
    </button>
  {/if}
</header>

<style>
  .settings-header {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding-bottom: 1.25rem;
    margin-bottom: 0.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .header-title {
    flex: 1;
    font-size: 0.9375rem;
    font-weight: 500;
    color: #e4e4e7;
    letter-spacing: -0.01em;
    margin: 0;
  }

  .close-btn {
    width: 1.75rem;
    height: 1.75rem;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: none;
    border-radius: 0.375rem;
    color: #3f3f46;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .close-btn:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #71717a;
  }

  .close-btn:active {
    background: rgba(255, 255, 255, 0.04);
  }

  /* Search input */
  .search-container {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 0.375rem;
    padding: 0.25rem 0.5rem;
    transition:
      border-color 0.15s ease,
      background 0.15s ease;
    flex: 0 1 12rem;
  }

  .search-container:focus-within {
    border-color: rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.06);
  }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: #d4d4d8;
    font-size: 0.75rem;
    min-width: 0;
  }

  .search-input::placeholder {
    color: #52525b;
  }

  .search-clear {
    background: none;
    border: none;
    color: #52525b;
    cursor: pointer;
    padding: 0.125rem;
    border-radius: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.15s ease;
  }

  .search-clear:hover {
    color: #71717a;
  }
</style>
