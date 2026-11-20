<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Setting Toggle
   *
   * Toggle switch for boolean settings.
   * Uses custom minimal toggle for consistency with flat design.
   */

  import type { Snippet } from 'svelte';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    /** Main label text */
    label: string;
    /** Optional hint/description */
    hint?: string;
    /** Optional warning gotcha, shown on amber ⚠ hover */
    warning?: string;
    /** Current checked state */
    checked: boolean;
    /** Callback when toggled */
    onchange: (value: boolean) => void;
    /** Show "Default" badge */
    isDefault?: boolean;
    /** Force disabled (e.g., forced-on Copy to Clipboard) */
    forceDisabled?: boolean;
    /** Optional docs page, opened by a 📖 button next to the label */
    docsUrl?: string;
    /** Tooltip / a11y label for the docs button */
    docsTooltip?: string;
    /** Optional inline content rendered next to the label (eg a minutes input) */
    afterLabel?: Snippet;
  }

  let {
    label,
    hint,
    warning,
    checked,
    onchange,
    docsUrl,
    docsTooltip,
    afterLabel,
    isDefault = false,
    forceDisabled = false,
  }: Props = $props();

  const isLocked = $derived(forceDisabled);
  const isActive = $derived(!isLocked && checked);

  function handleToggle() {
    if (!isLocked) {
      onchange(!checked);
    }
  }
</script>

<SettingRow
  {label}
  {hint}
  {warning}
  {docsUrl}
  {docsTooltip}
  {afterLabel}
  locked={isLocked}
  {isDefault}
>
  <button
    class="toggle-switch"
    class:active={isActive}
    class:disabled={isLocked}
    onclick={handleToggle}
    role="switch"
    aria-checked={isActive}
    aria-label={label}
    aria-disabled={isLocked}
  >
    <span class="toggle-thumb"></span>
  </button>
</SettingRow>

<style>
  .toggle-switch {
    position: relative;
    width: 2.25rem;
    height: 1.25rem;
    background: rgba(255, 255, 255, 0.08);
    border: none;
    border-radius: 0.625rem;
    cursor: pointer;
    transition: background 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    flex-shrink: 0;
  }

  .toggle-switch:hover:not(.disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .toggle-switch:active:not(.disabled) {
    background: rgba(255, 255, 255, 0.08);
  }

  .toggle-switch.active {
    background: rgba(132, 204, 22, 0.35);
  }

  .toggle-switch.active:hover {
    background: rgba(132, 204, 22, 0.45);
  }

  .toggle-switch.disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .toggle-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 0.875rem;
    height: 0.875rem;
    background: #52525b;
    border-radius: 50%;
    transition:
      left 0.2s cubic-bezier(0.34, 1.56, 0.64, 1),
      background 0.2s ease,
      box-shadow 0.2s ease;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  }

  .toggle-switch.active .toggle-thumb {
    left: calc(100% - 1.125rem);
    background: #84cc16;
    box-shadow: 0 0 8px rgba(132, 204, 22, 0.4);
  }
</style>
