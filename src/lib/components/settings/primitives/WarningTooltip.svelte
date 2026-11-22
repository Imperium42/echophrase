<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Warning Tooltip
   *
   * Amber triangle icon that reveals a tooltip on hover. Visually distinct from
   * HintTooltip's neutral "?" - use this to flag a gotcha the user should know
   * about before flipping a setting on.
   */

  interface Props {
    /** Tooltip text shown on hover */
    warning: string;
  }

  let { warning }: Props = $props();
</script>

<span class="warning-trigger">
  <svg
    class="warning-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path
      d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"
    />
    <line x1="12" y1="9" x2="12" y2="13" />
    <line x1="12" y1="17" x2="12.01" y2="17" />
  </svg>
  <span class="warning-tooltip">{warning}</span>
</span>

<style>
  .warning-trigger {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: help;
  }

  .warning-icon {
    width: 0.85rem;
    height: 0.85rem;
    color: #d97706;
    transition: color 0.15s ease;
  }

  .warning-trigger:hover .warning-icon {
    color: #f59e0b;
  }

  .warning-tooltip {
    position: absolute;
    left: 50%;
    top: calc(100% + 6px);
    transform: translateX(-50%);
    padding: 0.5rem 0.75rem;
    background: rgba(39, 39, 42, 0.95);
    border: 1px solid rgba(245, 158, 11, 0.35);
    border-radius: 0.375rem;
    font-size: 0.6875rem;
    font-weight: 400;
    color: #fcd34d;
    white-space: normal;
    max-width: 280px;
    min-width: 180px;
    text-align: center;
    line-height: 1.5;
    opacity: 0;
    visibility: hidden;
    transition:
      opacity 0.15s ease,
      visibility 0.15s ease;
    pointer-events: none;
    z-index: 100;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .warning-tooltip::after {
    content: '';
    position: absolute;
    bottom: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 5px solid transparent;
    border-bottom-color: rgba(39, 39, 42, 0.95);
  }

  .warning-trigger:hover .warning-tooltip {
    opacity: 1;
    visibility: visible;
  }
</style>
