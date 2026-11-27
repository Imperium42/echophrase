<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Overlay Position Picker
   *
   * A mock screen with six spots (corners, top and bottom center). The chosen
   * spot shows a tiny pill, the same shape as the real recording overlay.
   */

  import { OVERLAY_POSITIONS, type OverlayPosition } from '$lib/types/settings';
  import { t } from '$lib/locales';

  interface Props {
    value: OverlayPosition;
    disabled?: boolean;
    onchange: (position: OverlayPosition) => void;
  }

  let { value, disabled = false, onchange }: Props = $props();

  /** `top-left` -> `customize.overlayPositionTopLeft` */
  function labelFor(position: OverlayPosition): string {
    const suffix = position
      .split('-')
      .map(part => part[0].toUpperCase() + part.slice(1))
      .join('');
    return t(`customize.overlayPosition${suffix}`);
  }
</script>

<div
  class="screen"
  class:disabled
  role="radiogroup"
  aria-label={t('customize.overlayPositionAria')}
  aria-disabled={disabled}
>
  {#each OVERLAY_POSITIONS as position (position)}
    <button
      type="button"
      class="spot {position}"
      class:active={value === position}
      role="radio"
      aria-checked={value === position}
      aria-label={labelFor(position)}
      {disabled}
      onclick={() => onchange(position)}
    >
      <span class="mark"></span>
    </button>
  {/each}
  <span class="taskbar" aria-hidden="true"></span>
</div>

<style>
  .screen {
    position: relative;
    width: 9.5rem;
    aspect-ratio: 16 / 10;
    margin: 0.25rem 0 0.75rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.5rem;
    transition: opacity 0.15s ease;
  }

  .screen.disabled {
    opacity: 0.4;
  }

  .taskbar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 0.375rem;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 0 0 0.5rem 0.5rem;
  }

  .spot {
    position: absolute;
    display: grid;
    place-items: center;
    width: 1.75rem;
    height: 1.25rem;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 0.375rem;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .spot:disabled {
    cursor: default;
  }

  .spot:not(:disabled):hover {
    background: rgba(255, 255, 255, 0.04);
  }

  .top-left,
  .top-center,
  .top-right {
    top: 0.25rem;
  }
  .bottom-left,
  .bottom-center,
  .bottom-right {
    bottom: 0.625rem;
  }
  .top-left,
  .bottom-left {
    left: 0.25rem;
  }
  .top-right,
  .bottom-right {
    right: 0.25rem;
  }
  .top-center,
  .bottom-center {
    left: 50%;
    translate: -50% 0;
  }

  .mark {
    width: 0.375rem;
    height: 0.375rem;
    border-radius: 999px;
    background: #3f3f46;
    transition:
      width 0.2s cubic-bezier(0.22, 1, 0.36, 1),
      background 0.15s ease,
      box-shadow 0.15s ease;
  }

  .spot:not(:disabled):hover .mark {
    background: #a1a1aa;
  }

  .spot.active .mark {
    width: 1.125rem;
    background: #84cc16;
    box-shadow: 0 0 0 3px rgba(132, 204, 22, 0.12);
  }
</style>
