<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Debug Section
   *
   * Development-only debug tools (full first-launch reset).
   * Only rendered when import.meta.env.DEV is true.
   */

  import { SettingsSection } from '../primitives';
  import { t } from '$lib/locales';
  import { resetModelDetection } from '$lib/utils/modelAutoDetect';

  // Full reset state
  let isResetting = $state(false);
  let showResetConfirm = $state(false);

  /**
   * Full app reset - simulates first launch experience.
   * Clears local storage and resets model detection.
   * NOTE: Downloaded models are NOT deleted (saves bandwidth).
   */
  async function fullReset() {
    if (isResetting) return;
    isResetting = true;

    try {
      console.log('[DEBUG RESET] Starting full reset...');

      // 1. Reset model detection flag (will trigger auto-detect on next launch)
      // NOTE: Downloaded models are preserved to save bandwidth
      try {
        await resetModelDetection();
        console.log('[DEBUG RESET] Model detection reset (models preserved)');
      } catch (err) {
        console.warn('[DEBUG RESET] Model detection reset failed:', err);
      }

      // 2. Clear localStorage
      try {
        localStorage.clear();
        console.log('[DEBUG RESET] localStorage cleared');
      } catch (err) {
        console.warn('[DEBUG RESET] localStorage clear failed:', err);
      }

      console.log('[DEBUG RESET] Reset complete, reloading...');

      // 3. Reload the page to simulate fresh launch
      window.location.reload();
    } catch (error) {
      console.error('[DEBUG RESET] Reset failed:', error);
      isResetting = false;
    }
  }
</script>

{#if import.meta.env.DEV}
  <SettingsSection title={t('settings.debugTools')} icon="debug">
    <!-- Full Reset (simulates first launch) -->
    <div class="setting-row reset-row">
      <div class="setting-info">
        <span class="setting-label">{t('settings.fullReset')}</span>
        <span class="setting-hint">
          {t('settings.fullResetHint')}
        </span>
      </div>
      <div class="reset-btn-wrapper">
        {#if showResetConfirm}
          <div class="confirm-group">
            <span class="confirm-label">{t('settings.resetConfirm')}</span>
            <button
              class="debug-btn confirm-yes"
              onclick={() => {
                showResetConfirm = false;
                fullReset();
              }}
            >
              {t('common.yes')}
            </button>
            <button class="debug-btn confirm-no" onclick={() => (showResetConfirm = false)}>
              {t('common.no')}
            </button>
          </div>
        {:else}
          <button
            class="debug-btn danger"
            onclick={() => (showResetConfirm = true)}
            disabled={isResetting}
          >
            {isResetting ? t('settings.resetting') : t('settings.resetAll')}
          </button>
        {/if}
      </div>
    </div>
  </SettingsSection>
{/if}

<style>
  /* `.setting-row` + `.setting-label` come from app.css (shared) */

  .setting-info {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .setting-hint {
    font-size: 0.625rem;
    color: #52525b;
  }

  .debug-btn {
    padding: 0.25rem 0.5rem;
    background: rgba(59, 130, 246, 0.08);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.625rem;
    font-weight: 500;
    color: #60a5fa;
    cursor: pointer;
    /* Specific properties only - avoid "all" which causes repaint flicker */
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .debug-btn:hover:not(:disabled) {
    background: rgba(59, 130, 246, 0.15);
    /* No transform - avoids GPU layer flicker */
  }

  .debug-btn:active:not(:disabled) {
    background: rgba(59, 130, 246, 0.1);
  }

  .debug-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .debug-btn.danger {
    background: rgba(239, 68, 68, 0.08);
    color: #f87171;
  }

  .debug-btn.danger:hover:not(:disabled) {
    background: rgba(239, 68, 68, 0.15);
  }

  .confirm-group {
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  .confirm-label {
    font-size: 0.625rem;
    color: #f87171;
    font-weight: 500;
  }

  .debug-btn.confirm-yes {
    background: rgba(239, 68, 68, 0.15);
    color: #f87171;
  }

  .debug-btn.confirm-yes:hover {
    background: rgba(239, 68, 68, 0.25);
  }

  .debug-btn.confirm-no {
    background: rgba(113, 113, 122, 0.1);
    color: #a1a1aa;
  }

  .debug-btn.confirm-no:hover {
    background: rgba(113, 113, 122, 0.2);
  }
</style>
