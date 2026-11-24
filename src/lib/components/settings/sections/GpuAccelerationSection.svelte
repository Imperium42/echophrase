<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * GPU Acceleration Section
   *
   * Shows current execution provider status and lets the user override it.
   *
   * The default behavior is fully automatic: `with_auto_device(MaxPerformance)`
   * enumerates available devices at session creation time and picks the best
   * one (DirectML on Windows, CoreML on macOS, CPU on Linux/WSL).
   *
   * The radio buttons here let the user FORCE a specific provider when they want
   * (e.g. CPU for battery). The "Auto (Recommended)" option clears the override
   * and lets ort decide at runtime.
   */

  import { onMount } from 'svelte';
  import { SettingsSection } from '../primitives';
  import HintTooltip from '../primitives/HintTooltip.svelte';
  import { isWindows, isMac } from '$lib/utils/platform';
  import { t } from '$lib/locales';
  import { executionProviderStore } from '$lib/stores/executionProvider';
  import { isTauriAvailable } from '$lib/ipc/invoke';

  interface ExecutionProviderStatus {
    provider: string;
    available: boolean;
    reason: string;
    requires_download: boolean;
    /**
     * True when the hardware fundamentally can't run this provider. Hard-
     * incompatible providers are HIDDEN in the UI, not shown disabled.
     */
    hard_incompatible: boolean;
  }

  interface ExecutionProviderSelection {
    active: string;
    preferred: string | null;
    providers: ExecutionProviderStatus[];
    status_message: string;
    is_cpu_fallback: boolean;
  }

  let epStatus = $state<ExecutionProviderSelection | null>(null);
  let isLoading = $state(true);
  let selectError = $state<string | null>(null);
  // Track initial preferred to detect changes requiring restart
  let initialPreferred: string | null = $state(null);
  // Track user's selected provider ('auto' = no override)
  let selectedProvider: string = $state('auto');

  const isWindowsPlatform = isWindows();
  const isMacPlatform = isMac();

  const providerLabelKeys: Record<string, string> = {
    auto: 'gpu.providerAuto',
    direct_ml: 'gpu.providerDirectml',
    core_ml: 'gpu.providerCoreml',
    cpu: 'gpu.providerCpu',
  };

  const providerTooltipKeys: Record<string, string> = {
    auto: 'gpu.tooltipAuto',
    direct_ml: 'gpu.tooltipDirectml',
    core_ml: 'gpu.tooltipCoreml',
    cpu: 'gpu.tooltipCpu',
  };

  // Filter the backend-reported providers, drop CPU (handled separately), and
  // drop platform-incompatible + hardware-incompatible entries.
  //
  // Hide vs disable distinction:
  // - `hard_incompatible = true` → HIDE the option entirely.
  // - `available = false` (but not hard_incompatible) → keep the option VISIBLE
  //   but disabled, with the reason shown (eg CoreML needs macOS 13.3+).
  const platformProviders = $derived(
    (epStatus?.providers ?? []).filter(p => {
      if (p.provider === 'cpu') return false; // CPU added explicitly at the bottom
      if (p.provider === 'direct_ml' && !isWindowsPlatform) return false;
      if (p.provider === 'core_ml' && !isMacPlatform) return false;
      if (p.hard_incompatible) return false; // Hide hard-incompat options entirely
      return true;
    })
  );

  const needsRestart = $derived(
    initialPreferred !== null && selectedProvider !== (initialPreferred ?? 'auto')
  );

  // Selected row renders one badge; when the selection is pending a restart,
  // that same badge becomes the "Restart to apply" cue instead of "Selected".
  const selectedBadgeKey = $derived(needsRestart ? 'gpu.restartToApply' : 'gpu.selected');
  const selectedBadgeClass = $derived(needsRestart ? 'restart' : 'selected');

  onMount(async () => {
    if (!isTauriAvailable()) {
      isLoading = false;
      return;
    }
    const { invoke } = await import('@tauri-apps/api/core');

    // Use the EP store as the single source of truth. It's already initialized
    // during app init - no duplicate invoke needed. Subscribe reactively.
    // `unsub` is pre-declared so the callback can reference it even when the
    // store is already resolved and fires synchronously during subscribe()
    // (before the assignment completes) - avoids a TDZ ReferenceError.
    let unsub: (() => void) | undefined;
    let firedSync = false;
    unsub = executionProviderStore.subscribe(storeState => {
      if (storeState.isLoading) return;
      // Rebuild epStatus from store state + a separate get_execution_provider
      // call for the full providers list (fast - no ORT calls).
      invoke<ExecutionProviderSelection>('get_execution_provider')
        .then(ep => {
          epStatus = ep;
          initialPreferred = ep.preferred ?? 'auto';
          selectedProvider = ep.preferred ?? 'auto';
        })
        .catch(err => {
          console.error('[GpuAcceleration] Failed to load EP status:', err);
          if (isMacPlatform) {
            epStatus = {
              active: 'cpu',
              preferred: null,
              providers: [
                {
                  provider: 'core_ml',
                  available: false,
                  reason: t('gpu.requiresMacos13'),
                  requires_download: false,
                  hard_incompatible: false,
                },
                {
                  provider: 'cpu',
                  available: true,
                  reason: 'Always available',
                  requires_download: false,
                  hard_incompatible: false,
                },
              ],
              status_message: t('gpu.metalUnavailable'),
              is_cpu_fallback: true,
            };
            initialPreferred = 'auto';
            selectedProvider = 'auto';
          }
        })
        .finally(() => {
          isLoading = false;
        });
      // If subscribe() hasn't returned yet (synchronous first emission),
      // `unsub` is still undefined - defer the teardown until after assignment.
      if (unsub) unsub();
      else firedSync = true;
    });

    // Synchronous-fire case: the store resolved during subscribe(), so the
    // callback couldn't unsubscribe itself. Do it now that `unsub` is assigned.
    if (firedSync) unsub?.();
  });

  async function selectProvider(provider: string) {
    selectedProvider = provider;
    if (!isTauriAvailable()) return;
    const { invoke } = await import('@tauri-apps/api/core');
    try {
      // 'auto' clears the override; Rust translates it to execution_provider = None
      await invoke('set_preferred_execution_provider', { provider });
      selectError = null;
    } catch (err) {
      console.error('[GpuAcceleration] Failed to set provider:', err);
      selectError = String(err);
      setTimeout(() => (selectError = null), 5000);
    }
  }
</script>

<SettingsSection id="gpu-acceleration" title={t('gpu.title')} icon="gpu">
  {#if isLoading}
    <div class="loading-state">
      <div class="spinner"></div>
      <span>{t('gpu.detectingGpu')}</span>
    </div>
  {:else if epStatus}
    <!-- Inline active-device summary -->
    <div class="active-status">
      <span class="active-label">{t('gpu.active')}:</span>
      <span class="active-value" class:cpu={epStatus.is_cpu_fallback}>
        {providerLabelKeys[epStatus.active]
          ? t(providerLabelKeys[epStatus.active])
          : epStatus.active}
      </span>
    </div>

    <div class="ep-list">
      <!-- Auto (recommended) - clears the override -->
      <label class="ep-item" class:selected={selectedProvider === 'auto'}>
        <input
          type="radio"
          name="execution-provider"
          value="auto"
          checked={selectedProvider === 'auto'}
          onchange={() => selectProvider('auto')}
          class="ep-radio"
        />
        <div class="ep-item-header">
          <div class="ep-item-info">
            <span class="ep-item-name">
              {t('gpu.providerAuto')}
              <span class="recommended-tag">{t('gpu.recommended')}</span>
              <HintTooltip hint={t('gpu.tooltipAuto')} />
            </span>
            <span class="ep-item-reason">{t('gpu.providerAutoReason')}</span>
          </div>
          {#if selectedProvider === 'auto'}
            <span class="status-badge {selectedBadgeClass}">{t(selectedBadgeKey)}</span>
          {/if}
        </div>
      </label>

      {#each platformProviders as provider (provider.provider)}
        {@const isSelected = provider.provider === selectedProvider}
        {@const isIncompatible = !provider.available}
        <label class="ep-item" class:selected={isSelected} class:incompatible={isIncompatible}>
          <input
            type="radio"
            name="execution-provider"
            value={provider.provider}
            checked={isSelected}
            disabled={isIncompatible}
            onchange={() => selectProvider(provider.provider)}
            class="ep-radio"
          />
          <div class="ep-item-header">
            <div class="ep-item-info">
              <span class="ep-item-name">
                {providerLabelKeys[provider.provider]
                  ? t(providerLabelKeys[provider.provider])
                  : provider.provider}
                {#if providerTooltipKeys[provider.provider]}
                  <HintTooltip hint={t(providerTooltipKeys[provider.provider])} />
                {/if}
              </span>
              <span class="ep-item-reason">{provider.reason}</span>
            </div>
            {#if isSelected}
              <span class="status-badge {selectedBadgeClass}">{t(selectedBadgeKey)}</span>
            {:else if provider.available}
              <span class="status-badge available">{t('gpu.available')}</span>
            {:else}
              <span class="status-badge incompatible">{t('gpu.incompatible')}</span>
            {/if}
          </div>
        </label>
      {/each}

      <!-- CPU is always available as the explicit fallback -->
      <label class="ep-item" class:selected={selectedProvider === 'cpu'}>
        <input
          type="radio"
          name="execution-provider"
          value="cpu"
          checked={selectedProvider === 'cpu'}
          onchange={() => selectProvider('cpu')}
          class="ep-radio"
        />
        <div class="ep-item-header">
          <div class="ep-item-info">
            <span class="ep-item-name">
              {t('gpu.providerCpu')}
              <HintTooltip hint={t('gpu.tooltipCpu')} />
            </span>
            <span class="ep-item-reason">{t('gpu.providerCpuReason')}</span>
          </div>
          {#if selectedProvider === 'cpu'}
            <span class="status-badge {selectedBadgeClass}">{t(selectedBadgeKey)}</span>
          {:else}
            <span class="status-badge available">{t('gpu.available')}</span>
          {/if}
        </div>
      </label>
    </div>

    {#if selectError}
      <div class="error-toast">{selectError}</div>
    {/if}
  {:else}
    <div class="no-data">{t('gpu.unableToDetect')}</div>
  {/if}
</SettingsSection>

<style>
  .loading-state {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0;
    color: #52525b;
    font-size: 0.75rem;
  }

  .spinner {
    width: 0.875rem;
    height: 0.875rem;
    border: 1.5px solid rgba(255, 255, 255, 0.1);
    border-top-color: rgba(255, 255, 255, 0.4);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .active-status {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    padding: 0.5rem 0.625rem;
    margin-bottom: 0.5rem;
    /* Blue/info palette - distinct from the green "Selected" row state */
    background: rgba(59, 130, 246, 0.05);
    border: 1px solid rgba(59, 130, 246, 0.18);
    border-radius: 0.375rem;
    font-size: 0.75rem;
  }

  .active-label {
    color: #71717a;
    font-weight: 500;
  }

  .active-value {
    color: #93c5fd;
    font-weight: 500;
  }

  .active-value.cpu {
    /* Amber - emphasizes "you're on CPU fallback" */
    color: #fbbf24;
  }

  .active-device {
    color: #a1a1aa;
    font-weight: 400;
    margin-left: 0.25rem;
  }

  .ep-list {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .ep-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.625rem;
    border-radius: 0.375rem;
    opacity: 0.7;
    cursor: pointer;
    transition: all 0.15s;
    border: 1px solid transparent;
  }

  .ep-item:hover:not(.incompatible) {
    opacity: 1;
    background: rgba(255, 255, 255, 0.03);
  }

  .ep-item.selected {
    opacity: 1;
    background: rgba(34, 197, 94, 0.08);
    border: 1px solid rgba(34, 197, 94, 0.2);
  }

  .ep-item.incompatible {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .ep-radio {
    width: 1rem;
    height: 1rem;
    accent-color: #22c55e;
    cursor: pointer;
    flex-shrink: 0;
  }

  .ep-radio:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .ep-item-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    width: 100%;
  }

  .ep-item-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.0625rem;
  }

  .ep-item-name {
    font-size: 0.8125rem;
    font-weight: 500;
    color: #a1a1aa;
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  .ep-item.selected .ep-item-name {
    color: #e4e4e7;
  }

  .recommended-tag {
    font-size: 0.5625rem;
    padding: 0.0625rem 0.3125rem;
    border-radius: 0.1875rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    font-weight: 500;
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
  }

  .ep-item-reason {
    font-size: 0.625rem;
    color: #52525b;
  }

  .status-badge {
    font-size: 0.5625rem;
    padding: 0.125rem 0.3125rem;
    border-radius: 0.1875rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    font-weight: 500;
    flex-shrink: 0;
    border: none;
  }

  .status-badge.selected {
    background: rgba(34, 197, 94, 0.15);
    color: #22c55e;
  }

  .status-badge.available {
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
  }

  .status-badge.incompatible {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
  }

  /* Amber - inherits the "you need to act" semantic from the old restart banner */
  .status-badge.restart {
    background: rgba(234, 179, 8, 0.15);
    color: #fbbf24;
  }

  .error-toast {
    margin-top: 0.5rem;
    padding: 0.375rem 0.5rem;
    font-size: 0.6875rem;
    color: #fbbf24;
    background: rgba(251, 191, 36, 0.08);
    border: 1px solid rgba(251, 191, 36, 0.2);
    border-radius: 0.25rem;
  }

  .no-data {
    font-size: 0.75rem;
    color: #52525b;
    padding: 0.5rem 0;
  }
</style>
