<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Language Selector Component
   *
   * Globe icon button with dropdown for switching languages.
   * Positioned in the top-right area, next to the other header buttons.
   *
   * Features:
   * - Globe icon with tooltip showing "Language 語言"
   * - Click to open dropdown with language options
   * - Current language indicated with checkmark
   * - Persists selection to Tauri settings
   */

  import { onMount, onDestroy } from 'svelte';
  import { locale, setLocale, t, LOCALES, type SupportedLocale } from '$lib/locales';
  import { modalController } from '$lib/stores/modalController';
  import { Icon } from '$lib/icons';

  let dropdownOpen = false;
  let menuButton: HTMLButtonElement | null = null;
  let dropdownMenu: HTMLDivElement | null = null;
  let modalId: string | null = null;
  let isHovered = false;

  function toggleDropdown() {
    dropdownOpen = !dropdownOpen;

    if (dropdownOpen) {
      modalId = modalController.push({
        onClose: closeDropdown,
        priority: 1,
      });
    } else {
      closeDropdown();
    }
  }

  function closeDropdown() {
    dropdownOpen = false;
    if (modalId) {
      modalController.pop(modalId);
      modalId = null;
    }
  }

  function handleClickOutside(event: MouseEvent) {
    if (!dropdownOpen) return;

    const target = event.target as Node;
    if (
      menuButton &&
      !menuButton.contains(target) &&
      dropdownMenu &&
      !dropdownMenu.contains(target)
    ) {
      closeDropdown();
    }
  }

  function selectLanguage(code: SupportedLocale) {
    setLocale(code);
    closeDropdown();
  }

  onMount(() => {
    document.addEventListener('click', handleClickOutside);
  });

  onDestroy(() => {
    document.removeEventListener('click', handleClickOutside);
    if (modalId) {
      modalController.pop(modalId);
    }
  });
</script>

<div class="relative">
  <!-- Globe Button with Tooltip -->
  <div class="relative">
    <button
      bind:this={menuButton}
      on:click={toggleDropdown}
      on:mouseenter={() => (isHovered = true)}
      on:mouseleave={() => (isHovered = false)}
      class="w-10 h-10 flex items-center justify-center rounded-lg transition-colors duration-200 ease-out focus:outline-none focus:ring-2 focus:ring-zinc-600 cursor-pointer {dropdownOpen
        ? 'bg-zinc-800 text-zinc-200'
        : 'bg-zinc-800/50 text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200'}"
      aria-label={t('languageSelector.tooltip')}
      aria-expanded={dropdownOpen}
    >
      <Icon name="globe" class="w-5 h-5" />
    </button>

    <!-- Tooltip (shows on hover, hides when dropdown open) -->
    {#if isHovered && !dropdownOpen}
      <div
        class="absolute top-full right-0 mt-2 px-3 py-1.5 bg-zinc-800 border border-zinc-700 rounded-lg shadow-lg whitespace-nowrap pointer-events-none animate-fadeIn z-50"
      >
        <span class="text-xs text-zinc-300">{t('languageSelector.tooltip')}</span>
        <!-- Tooltip arrow -->
        <div
          class="absolute -top-1.5 right-4 w-3 h-3 bg-zinc-800 border-l border-t border-zinc-700 transform rotate-45"
        ></div>
      </div>
    {/if}
  </div>

  <!-- Dropdown Menu -->
  {#if dropdownOpen}
    <div
      bind:this={dropdownMenu}
      class="absolute top-full right-0 mt-3 w-56 bg-zinc-900 border border-zinc-800 rounded-xl shadow-2xl overflow-hidden animate-slideIn"
      style="z-index: 9100;"
    >
      <!-- Dropdown tail/arrow -->
      <div class="dropdown-tail"></div>

      <!-- Header -->
      <div class="px-4 py-3 border-b border-zinc-800">
        <div class="flex items-center gap-2 text-sm text-zinc-300">
          <Icon name="globe" class="w-4 h-4 text-zinc-500" />
          <span>{t('common.language')}</span>
        </div>
      </div>

      <!-- Language Options -->
      <div class="py-1">
        {#each LOCALES as localeInfo (localeInfo.code)}
          <button
            on:click={() => selectLanguage(localeInfo.code)}
            class="w-full px-4 py-2.5 flex items-center justify-between text-sm transition-colors duration-150 cursor-pointer {locale.current ===
            localeInfo.code
              ? 'bg-zinc-800/50 text-zinc-100'
              : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'}"
          >
            <span class="flex items-center gap-2 whitespace-nowrap">
              <span>{localeInfo.name}</span>
              {#if localeInfo.code !== 'en'}
                <span class="text-xs text-zinc-500">({localeInfo.englishName})</span>
              {/if}
            </span>

            <!-- Checkmark for selected -->
            {#if locale.current === localeInfo.code}
              <Icon name="check" class="w-4 h-4 text-lime-400" />
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateY(-8px) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .animate-slideIn {
    animation: slideIn 0.15s ease-out;
  }

  .animate-fadeIn {
    animation: fadeIn 0.15s ease-out;
  }

  .dropdown-tail {
    position: absolute;
    top: -6px;
    right: 14px;
    width: 12px;
    height: 12px;
    background: #18181b; /* zinc-900 */
    border-left: 1px solid #27272a; /* zinc-800 */
    border-top: 1px solid #27272a;
    transform: rotate(45deg);
    z-index: 1;
  }
</style>
