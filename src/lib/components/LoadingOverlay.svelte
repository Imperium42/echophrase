<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { onDestroy } from 'svelte';
  import { Z_INDEX, setGlobalOverlayBlocking } from '$lib/stores/modalController';

  /**
   * Full-screen loading overlay with modern water-like spinner
   *
   * Features:
   * - Violet-tinted blur backdrop matching app theme
   * - Smooth water-like flowing spinner animation
   * - Optional message text
   * - Success state with smooth green color transition
   * - Welcome message with user name on success
   * - Reusable for any loading state
   * - z-index: GLOBAL_OVERLAY (9999) - blocks all modals, below debug tools
   * - Blocks ESC key while visible to prevent accidental modal closures
   */

  export let visible: boolean = false;
  export let message: string = 'Loading...';
  export let subtext: string | null = null;
  export let success: boolean = false;
  export let fadingOut: boolean = false;
  export let userName: string | null = null;

  $: setGlobalOverlayBlocking(visible);

  onDestroy(() => {
    setGlobalOverlayBlocking(false);
  });
</script>

{#if visible}
  <div
    class="loading-overlay"
    class:fading-out={fadingOut}
    role="status"
    aria-live="polite"
    aria-label={message}
    style="--z-global-overlay: {Z_INDEX.BLOCKING};"
  >
    <!-- Backdrop with violet-tinted blur, transitions to green on success -->
    <div class="backdrop" class:success class:fading-out={fadingOut}></div>

    <!-- Spinner container -->
    <div class="spinner-container" class:fading-out={fadingOut}>
      <!-- Modern water-like flowing spinner -->
      <div class="spinner">
        <svg viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
          <!-- Outer flowing ring -->
          <circle
            class="spinner-ring"
            cx="50"
            cy="50"
            r="40"
            fill="none"
            stroke="url(#gradient1)"
            stroke-width="3"
            stroke-linecap="round"
            stroke-dasharray="20 200"
            stroke-dashoffset="0"
          />

          <!-- Inner flowing ring -->
          <circle
            class="spinner-ring-inner"
            cx="50"
            cy="50"
            r="32"
            fill="none"
            stroke="url(#gradient2)"
            stroke-width="2"
            stroke-linecap="round"
            stroke-dasharray="15 150"
            stroke-dashoffset="0"
          />

          <!-- Gradient definitions for water-like effect with smooth color transitions -->
          <defs>
            <linearGradient id="gradient1" x1="0%" y1="0%" x2="100%" y2="100%">
              <stop offset="0%" style="stop-color:{success ? '#10b981' : '#00D4FF'};stop-opacity:1">
                <animate attributeName="stop-color" dur="300ms" fill="freeze" />
              </stop>
              <stop
                offset="50%"
                style="stop-color:{success ? '#34d399' : '#667eea'};stop-opacity:0.8"
              >
                <animate attributeName="stop-color" dur="300ms" fill="freeze" />
              </stop>
              <stop
                offset="100%"
                style="stop-color:{success ? '#6ee7b7' : '#764ba2'};stop-opacity:0.6"
              >
                <animate attributeName="stop-color" dur="300ms" fill="freeze" />
              </stop>
            </linearGradient>
            <linearGradient id="gradient2" x1="100%" y1="0%" x2="0%" y2="100%">
              <stop
                offset="0%"
                style="stop-color:{success ? '#6ee7b7' : '#764ba2'};stop-opacity:0.6"
              >
                <animate attributeName="stop-color" dur="300ms" fill="freeze" />
              </stop>
              <stop
                offset="50%"
                style="stop-color:{success ? '#34d399' : '#667eea'};stop-opacity:0.8"
              >
                <animate attributeName="stop-color" dur="300ms" fill="freeze" />
              </stop>
              <stop
                offset="100%"
                style="stop-color:{success ? '#10b981' : '#00D4FF'};stop-opacity:1"
              >
                <animate attributeName="stop-color" dur="300ms" fill="freeze" />
              </stop>
            </linearGradient>
          </defs>
        </svg>
      </div>

      <!-- Loading message (or welcome message on success) -->
      {#if success && userName}
        <p class="message">Welcome, {userName}</p>
      {:else if message}
        <p class="message">{message}</p>
      {/if}

      <!-- Subtext (polling indicator, etc.) -->
      {#if subtext && !success}
        <p class="subtext">{subtext}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .loading-overlay {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-global-overlay); /* Above modals, below debug tools */
    pointer-events: auto;
    animation: fadeIn 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    /* Isolate to prevent repaints affecting main UI */
    contain: layout style paint;
    isolation: isolate;
  }

  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  /* Violet-tinted backdrop with smooth transition to green on success */
  .backdrop {
    position: absolute;
    inset: 0;
    /* Solid dark tinted background - no expensive backdrop-filter blur */
    background: rgba(24, 10, 50, 0.92); /* Deep violet-black */
    transition:
      background 800ms cubic-bezier(0.16, 1, 0.3, 1),
      opacity 600ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .backdrop.success {
    background: rgba(10, 40, 30, 0.92); /* Deep green-black */
  }

  .backdrop.fading-out {
    opacity: 0;
  }

  .spinner-container {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 24px;
    z-index: 1;
    transition:
      opacity 600ms cubic-bezier(0.16, 1, 0.3, 1),
      transform 600ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  .spinner-container.fading-out {
    opacity: 0;
    transform: scale(0.95);
  }

  /* Spinner with water-like flowing animation */
  .spinner {
    width: 120px;
    height: 120px;
    filter: drop-shadow(0 0 20px rgba(0, 212, 255, 0.3));
  }

  .spinner svg {
    width: 100%;
    height: 100%;
    transform-origin: center;
    animation: rotate 4s linear infinite;
  }

  /* Outer ring - slower, clockwise */
  .spinner-ring {
    transform-origin: center;
    animation: flow 3s ease-in-out infinite;
  }

  /* Inner ring - faster, counter-clockwise */
  .spinner-ring-inner {
    transform-origin: center;
    animation: flowReverse 2s ease-in-out infinite;
  }

  @keyframes rotate {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  /* Flowing water-like dash animation */
  @keyframes flow {
    0% {
      stroke-dashoffset: 0;
    }
    50% {
      stroke-dashoffset: 110;
    }
    100% {
      stroke-dashoffset: 220;
    }
  }

  @keyframes flowReverse {
    0% {
      stroke-dashoffset: 0;
    }
    50% {
      stroke-dashoffset: -82.5;
    }
    100% {
      stroke-dashoffset: -165;
    }
  }

  .message {
    margin: 0;
    font-size: 16px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
    text-align: center;
    letter-spacing: 0.02em;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.9;
    }
    50% {
      opacity: 0.6;
    }
  }

  .subtext {
    margin: -12px 0 0 0;
    font-size: 13px;
    font-weight: 400;
    color: rgba(255, 255, 255, 0.5);
    text-align: center;
    letter-spacing: 0.01em;
  }
</style>
