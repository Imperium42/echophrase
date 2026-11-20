<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Settings Footer
   *
   * Minimal footer with text-style Reset and subtle Done button.
   */

  import { t } from '$lib/locales';

  interface Props {
    /** Reset to defaults handler */
    onReset: () => void;
    /** Done/close handler */
    onDone: () => void;
  }

  let { onReset, onDone }: Props = $props();

  // Confirmation state for reset button
  let confirmingReset = $state(false);
  let confirmTimeout: ReturnType<typeof setTimeout> | null = null;

  function handleResetClick() {
    if (confirmingReset) {
      // Second click - actually reset
      if (confirmTimeout) clearTimeout(confirmTimeout);
      confirmingReset = false;
      onReset();
    } else {
      // First click - show confirmation
      confirmingReset = true;
      // Auto-cancel after 3s
      confirmTimeout = setTimeout(() => {
        confirmingReset = false;
      }, 3000);
    }
  }

  function handleResetBlur() {
    // Cancel confirmation if user clicks elsewhere
    if (confirmTimeout) clearTimeout(confirmTimeout);
    confirmingReset = false;
  }
</script>

<footer class="settings-footer">
  <button
    class="btn-reset"
    class:confirming={confirmingReset}
    onclick={handleResetClick}
    onblur={handleResetBlur}
  >
    {confirmingReset ? t('settingsFooter.confirmReset') : t('settingsFooter.reset')}
  </button>
  <button class="btn-done" onclick={onDone}>{t('settingsFooter.done')}</button>
</footer>

<style>
  .settings-footer {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 0.75rem;
    padding-top: 1.25rem;
    margin-top: 0.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
  }

  .btn-reset {
    padding: 0.5rem 0.875rem;
    background: transparent;
    border: none;
    border-radius: 0.375rem;
    font-size: 0.8125rem;
    font-weight: 450;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .btn-reset:hover {
    color: #ef4444;
    background: rgba(239, 68, 68, 0.1);
  }

  .btn-reset:active {
    color: #71717a;
    background: rgba(239, 68, 68, 0.05);
  }

  .btn-reset.confirming {
    color: #ef4444;
    background: rgba(239, 68, 68, 0.15);
  }

  .btn-done {
    padding: 0.5rem 1rem;
    background: rgba(255, 255, 255, 0.08);
    border: none;
    border-radius: 0.375rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: #e4e4e7;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .btn-done:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .btn-done:active {
    background: rgba(255, 255, 255, 0.06);
  }
</style>
