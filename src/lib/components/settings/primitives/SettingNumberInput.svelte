<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * SettingNumberInput
   *
   * Small inline whole-number field with optional prefix/suffix text, eg
   * "Idle for [10] mins". Meant for a SettingToggle's `afterLabel` snippet.
   * Typing edits a local draft (an empty field mid-edit never writes junk); the
   * value is clamped to min-max and committed on change/blur only.
   */

  interface Props {
    /** Current saved value (undefined = use `fallback`) */
    value: number | undefined;
    /** Value shown when nothing is saved yet */
    fallback: number;
    min: number;
    max: number;
    /** Text before the field */
    prefix?: string;
    /** Text after the field */
    suffix?: string;
    disabled?: boolean;
    ariaLabel: string;
    /** Called with the clamped value, only when it differs from the saved one */
    oncommit: (value: number) => void;
  }

  let {
    value,
    fallback,
    min,
    max,
    prefix,
    suffix,
    disabled = false,
    ariaLabel,
    oncommit,
  }: Props = $props();

  const current = $derived(value ?? fallback);
  let draft = $derived(String(current));

  function commit() {
    const parsed = Math.round(Number(draft));
    const next = Number.isFinite(parsed) ? Math.min(max, Math.max(min, parsed)) : current;
    draft = String(next);
    if (next !== current) oncommit(next);
  }
</script>

{#if prefix}<span class="affix" class:dim={disabled}>{prefix}</span>{/if}
<input
  type="number"
  class="number-input"
  {min}
  {max}
  step="1"
  inputmode="numeric"
  bind:value={draft}
  onchange={commit}
  onblur={commit}
  {disabled}
  aria-label={ariaLabel}
/>
{#if suffix}<span class="affix" class:dim={disabled}>{suffix}</span>{/if}

<style>
  .number-input {
    width: 3rem;
    padding: 0.25rem 0.375rem;
    appearance: textfield;
    -moz-appearance: textfield;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.375rem;
    color: #e4e4e7;
    font-size: 0.8125rem;
    text-align: center;
  }

  /* Hide the browser's native spinner - it renders as a bright block on the dark theme. */
  .number-input::-webkit-inner-spin-button,
  .number-input::-webkit-outer-spin-button {
    -webkit-appearance: none;
    appearance: none;
    margin: 0;
  }

  .number-input:focus-visible {
    outline: 2px solid rgba(132, 204, 22, 0.6);
    outline-offset: 1px;
  }

  .number-input:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .affix {
    font-size: 0.8125rem;
    color: #d4d4d8;
  }

  .affix.dim {
    color: #71717a;
  }
</style>
