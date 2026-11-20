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
   * Since the ort 2.0.0-rc.12 upgrade, the default behavior is fully automatic:
   * `with_auto_device(MaxPerformance)` enumerates available devices at session
   * creation time and picks the best one (CUDA > DirectML > CoreML > NPU > CPU).
   *
   * The radio buttons here let the user FORCE a specific provider when they want
   * (e.g. CPU for battery, or CUDA when ort would otherwise pick DirectML on
   * Windows). The "Auto (Recommended)" option clears the override and lets ort
   * decide at runtime.
   */

  import { onMount, onDestroy } from 'svelte';
  import { SettingsSection } from '../primitives';
  import HintTooltip from '../primitives/HintTooltip.svelte';
  import ModalCloseButton from '$lib/components/ModalCloseButton.svelte';
  import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
  } from '$lib/components/ui/collapsible';
  import { isWindows, isMac } from '$lib/utils/platform';
  import { CUDA_PLUGIN_ENABLED } from '$lib/config/featureFlags';
  import { t } from '$lib/locales';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { executionProviderStore } from '$lib/stores/executionProvider';
  import { Icon } from '$lib/icons';
  import { useClipboard } from '$lib/composables/useClipboard';
  import { isTauriAvailable } from '$lib/ipc/invoke';
  import { useCudaPlugin } from '$lib/composables/useCudaPlugin';

  const clipboard = useClipboard();
  let cmdCopied = $state(false);
  let cmdCopyTimeout: ReturnType<typeof setTimeout> | null = null;

  function copyCmd(text: string) {
    clipboard.copyWithFeedback(text, () => {
      cmdCopied = true;
      if (cmdCopyTimeout) clearTimeout(cmdCopyTimeout);
      cmdCopyTimeout = setTimeout(() => {
        cmdCopied = false;
      }, 2000);
    });
  }

  interface ExecutionProviderStatus {
    provider: string;
    available: boolean;
    reason: string;
    requires_download: boolean;
    /**
     * True when the hardware fundamentally can't run this provider (eg CUDA
     * on a non-NVIDIA machine). Hard-incompatible providers are HIDDEN in
     * the UI, not shown disabled -- there's nothing the user can install to
     * fix it. Soft-incompatible (eg NVIDIA GPU present but CUDA/cuDNN
     * missing) stays visible-but-disabled so the user knows what to install.
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

  interface DeviceInfo {
    ep_name: string;
    ep_vendor: string;
    hw_vendor: string;
    device_type: string;
    device_id: number;
  }

  /** Result of testing one CUDA-related env var (mirrors Rust `CudaEnvVarTest`). */
  interface CudaEnvVarTest {
    name: string;
    actual: string;
    ok: boolean;
  }

  let epStatus = $state<ExecutionProviderSelection | null>(null);
  let devices = $state<DeviceInfo[]>([]);
  let isLoading = $state(true);
  let selectError = $state<string | null>(null);
  let showCudaHelp = $state(false);
  // Path-test results. Null = not tested yet (modal hasn't auto-run the check).
  let cudaPathTests = $state<CudaEnvVarTest[] | null>(null);
  // Per-var spinner state (set while that specific row is re-checking).
  let refreshingVar = $state<string | null>(null);
  // Collapsible "Path validation instructions" - closed by default per plan.
  let showPathInstructions = $state(false);

  // CUDA plugin (downloaded DLC) lifecycle - state, IPC, event listeners
  // owned by the composable. Keeps this file focused on EP UI.
  const cudaPlugin = useCudaPlugin();
  const pluginStatus = cudaPlugin.status;
  const pluginDownload = cudaPlugin.download;
  const pluginError = cudaPlugin.error;

  // Both env vars green = DLC button unlocks
  const allRequirementsMet = $derived(
    !!cudaPathTests && cudaPathTests.length >= 2 && cudaPathTests.every(t => t.ok)
  );

  async function testCudaPaths(specificVar?: string) {
    if (!isTauriAvailable()) return;
    if (specificVar) refreshingVar = specificVar;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const result = await invoke<CudaEnvVarTest[]>('test_cuda_env_paths');
      // Guard: only write state if the modal is still open. The invoke is
      // async and can resolve after `showCudaHelp` flips to false; writing
      // reactive state during {#if} teardown causes a TDZ in the generated
      // subscriber that re-evaluates the {#each} subtree.
      if (showCudaHelp) cudaPathTests = result;
    } catch (err) {
      console.error('[GpuAcceleration] test_cuda_env_paths failed:', err);
      if (showCudaHelp) cudaPathTests = [];
    } finally {
      refreshingVar = null;
    }
  }

  function cudaTestFor(name: string): CudaEnvVarTest | undefined {
    return cudaPathTests?.find(t => t.name === name);
  }

  const refreshPluginStatus = cudaPlugin.refresh;
  const installPlugin = cudaPlugin.install;
  const cancelPluginInstall = cudaPlugin.cancel;
  const uninstallPlugin = cudaPlugin.uninstall;

  async function restartApp() {
    if (!isTauriAvailable()) return;
    try {
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } catch (err) {
      console.error('[GpuAcceleration] relaunch failed:', err);
    }
  }

  function closeCudaHelp() {
    // Don't null `cudaPathTests` here — derived/subscribers inside the
    // modal subtree race the teardown and throw "Cannot access
    // uninitialized variable". The next open overwrites it via
    // testCudaPaths() so leaving stale data is harmless.
    showCudaHelp = false;
    showPathInstructions = false;
    cudaPlugin.clearError();
  }

  // Auto-check paths + fetch plugin status whenever the modal opens.
  $effect(() => {
    if (showCudaHelp) {
      testCudaPaths();
      refreshPluginStatus();
    }
  });

  // Track initial preferred to detect changes requiring restart
  let initialPreferred: string | null = $state(null);
  // Track user's selected provider ('auto' = no override)
  let selectedProvider: string = $state('auto');

  const isWindowsPlatform = isWindows();
  const isMacPlatform = isMac();

  const providerLabelKeys: Record<string, string> = {
    auto: 'gpu.providerAuto',
    direct_ml: 'gpu.providerDirectml',
    cuda: 'gpu.providerCuda',
    core_ml: 'gpu.providerCoreml',
    cpu: 'gpu.providerCpu',
  };

  const providerTooltipKeys: Record<string, string> = {
    auto: 'gpu.tooltipAuto',
    direct_ml: 'gpu.tooltipDirectml',
    cuda: 'gpu.tooltipCuda',
    core_ml: 'gpu.tooltipCoreml',
    cpu: 'gpu.tooltipCpu',
  };

  // Filter the backend-reported providers, drop CPU (handled separately), and
  // drop platform-incompatible + hardware-incompatible entries.
  //
  // Hide vs disable distinction:
  // - `hard_incompatible = true` → HIDE the option entirely. The user's hardware
  //   fundamentally can't run it (eg CUDA on a machine with no NVIDIA GPU).
  //   No install can fix it, so showing a greyed-out option just adds noise.
  // - `available = false` (but not hard_incompatible) → keep the option VISIBLE
  //   but disabled, with a hint like "Install cuDNN to enable". The user CAN fix
  //   it by downloading the right SDK.
  const platformProviders = $derived(
    (epStatus?.providers ?? []).filter(p => {
      if (p.provider === 'cpu') return false; // CPU added explicitly at the bottom
      if (p.provider === 'direct_ml' && !isWindowsPlatform) return false;
      if (p.provider === 'cuda' && isMacPlatform) return false;
      if (p.provider === 'core_ml' && !isMacPlatform) return false;
      if (p.hard_incompatible) return false; // Hide hard-incompat options entirely
      // Feature-flag: CUDA plugin disabled on Win+Mac pending ORT 1.25 stable (see
      // src/lib/config/featureFlags.ts). Linux still shows CUDA because its
      // production build links CUDA at compile time and doesn't use the plugin.
      if (p.provider === 'cuda' && !CUDA_PLUGIN_ENABLED && (isWindowsPlatform || isMacPlatform))
        return false;
      return true;
    })
  );

  // Find the current active device for the inline status line
  const activeDevice = $derived(
    devices.find(d => {
      const ep = d.ep_name.toLowerCase();
      if (epStatus?.active === 'cuda') return ep === 'cudaexecutionprovider';
      if (epStatus?.active === 'direct_ml') return ep === 'dmlexecutionprovider';
      if (epStatus?.active === 'core_ml') return ep === 'coremlexecutionprovider';
      return false;
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
    cudaPlugin.attachListeners();

    // Use the EP store as the single source of truth. It's already initialized
    // during app init — no duplicate invoke needed. Subscribe reactively.
    // `unsub` is pre-declared so the callback can reference it even when the
    // store is already resolved and fires synchronously during subscribe()
    // (before the assignment completes) — avoids a TDZ ReferenceError.
    let unsub: (() => void) | undefined;
    let firedSync = false;
    unsub = executionProviderStore.subscribe(storeState => {
      if (storeState.isLoading) return;
      // Rebuild epStatus from store state + a separate get_execution_provider
      // call for the full providers list (fast — no ORT calls).
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
      // `unsub` is still undefined — defer the teardown until after assignment.
      if (unsub) unsub();
      else firedSync = true;
    });

    // Synchronous-fire case: the store resolved during subscribe(), so the
    // callback couldn't unsubscribe itself. Do it now that `unsub` is assigned.
    if (firedSync) unsub?.();
  });

  // Drain Tauri event listeners on unmount. Without this, every settings
  // open/close cycle leaks 3 listeners (`ort-plugin-progress/-complete/-error`),
  // compounding close-time work as their callbacks pile up.
  onDestroy(() => {
    cudaPlugin.cleanup();
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

  function deviceLabel(d: DeviceInfo): string {
    // "NVIDIA (GPU)" or "Microsoft (CPU)"
    return `${d.hw_vendor} (${d.device_type})`;
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
              <span class="tooltip-trigger" title={t('gpu.tooltipAuto')}>?</span>
            </span>
            <span class="ep-item-reason">{t('gpu.providerAutoReason')}</span>
          </div>
          {#if selectedProvider === 'auto'}
            <span class="status-badge {selectedBadgeClass}">{t(selectedBadgeKey)}</span>
          {/if}
        </div>
      </label>

      {#each platformProviders as provider}
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
                  <span class="tooltip-trigger" title={t(providerTooltipKeys[provider.provider])}
                    >?</span
                  >
                {/if}
              </span>
              <span class="ep-item-reason">{provider.reason}</span>
            </div>
            {#if isSelected}
              <span class="status-badge {selectedBadgeClass}">{t(selectedBadgeKey)}</span>
            {:else if provider.available}
              <span class="status-badge available">{t('gpu.available')}</span>
            {:else}
              <button
                type="button"
                class="status-badge incompatible clickable"
                onclick={e => {
                  e.preventDefault();
                  showCudaHelp = true;
                }}>{t('gpu.incompatible')}</button
              >
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
              <span class="tooltip-trigger" title={t('gpu.tooltipCpu')}>?</span>
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

<svelte:window
  onkeydown={e => {
    if (showCudaHelp && CUDA_PLUGIN_ENABLED && e.key === 'Escape') closeCudaHelp();
  }}
/>

{#if showCudaHelp && CUDA_PLUGIN_ENABLED}
  <!-- Feature-flag guard: even if showCudaHelp is flipped true somehow, the modal
	     cannot render when CUDA_PLUGIN_ENABLED is false. See src/lib/config/featureFlags.ts. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="cuda-modal-overlay" role="presentation" onclick={closeCudaHelp}>
    <div
      class="cuda-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="cuda-modal-title"
      tabindex="-1"
      onclick={e => e.stopPropagation()}
      onkeydown={e => e.stopPropagation()}
    >
      <ModalCloseButton onClose={closeCudaHelp} />
      <div id="cuda-modal-title" class="cuda-modal-title">{t('gpu.cudaSetupTitle')}</div>

      <div class="cuda-modal-body">
        {#if !isWindowsPlatform}
          <div class="cuda-modal-note">{t('gpu.cudaLinuxApt')}</div>
          <div class="cuda-cmd-block">
            <code class="cuda-cmd">sudo apt install cuda-toolkit-13-0 libcudnn9-cuda-13</code>
            <button
              class="cuda-copy-btn"
              class:copied={cmdCopied}
              title={cmdCopied ? 'Copied!' : 'Copy'}
              onclick={() => copyCmd('sudo apt install cuda-toolkit-13-0 libcudnn9-cuda-13')}
            >
              {#if cmdCopied}
                <Icon name="check" size={11} />
              {:else}
                <Icon name="copy" size={11} />
              {/if}
            </button>
          </div>
        {/if}
        <!-- Installer links (Win11 installer args pre-selected; CUDA toolkit bundles the driver) -->
        <div class="cuda-links-row">
          <button
            class="cuda-link"
            onclick={() =>
              launchUrl(
                isWindowsPlatform
                  ? 'https://developer.nvidia.com/cuda-downloads?target_os=Windows&target_arch=x86_64&target_version=11&target_type=exe_network'
                  : 'https://developer.nvidia.com/cuda-downloads?target_os=Linux'
              )}
          >
            <span>CUDA Toolkit 13.0+</span>
            <Icon name="external-link" size={11} />
          </button>
          <button
            class="cuda-link"
            onclick={() =>
              launchUrl(
                isWindowsPlatform
                  ? 'https://developer.nvidia.com/cudnn-downloads?target_os=Windows&target_arch=x86_64&target_version=11&target_type=exe_local'
                  : 'https://developer.nvidia.com/cudnn-downloads?target_os=Linux'
              )}
          >
            <span>cuDNN 9.19+</span>
            <Icon name="external-link" size={11} />
          </button>
        </div>
      </div>

      {#if isWindowsPlatform && cudaPathTests}
        <div class="cuda-env-block">
          {#each [{ name: 'CUDA_PATH', hint: '%ProgramFiles%\\NVIDIA GPU Computing Toolkit\\CUDA\\v13.x' }, { name: 'CUDNN_PATH', hint: '%ProgramFiles%\\NVIDIA\\CUDNN\\v9.x' }] as row}
            {@const res = cudaTestFor(row.name)}
            <div class="cuda-env-row">
              <div class="cuda-env-header">
                <button
                  class="env-refresh"
                  disabled={refreshingVar === row.name}
                  title={t('gpu.cudaRecheck', { name: row.name })}
                  onclick={() => testCudaPaths(row.name)}
                >
                  <Icon
                    name="refresh"
                    size={10}
                    class={refreshingVar === row.name ? 'spinning' : ''}
                  />
                </button>
                <code class="cuda-env-name">{row.name}</code>
              </div>
              <code class="cuda-env-value">{row.hint}</code>
              {#if res}
                <code class="cuda-env-actual" class:ok={res.ok} class:bad={!res.ok}>
                  Actual: {res.actual || '(unset)'}
                </code>
              {/if}
            </div>
          {/each}
        </div>

        <!-- Requirements checklist: hover "Requirements" text for TLDR, click to open instructions -->
        <div class="cuda-requirements">
          <span class="requirements-label">
            <button
              class="requirements-link"
              onclick={() => (showPathInstructions = !showPathInstructions)}
            >
              {t('gpu.cudaRequirements')}
            </button>
            <HintTooltip hint={t('gpu.cudaRequirementsTooltip')} />
            :
          </span>
          <span
            class="req-item"
            class:ok={cudaTestFor('CUDA_PATH')?.ok}
            class:bad={cudaTestFor('CUDA_PATH') && !cudaTestFor('CUDA_PATH')!.ok}
          >
            {#if cudaTestFor('CUDA_PATH')?.ok}
              <Icon name="check-circle" size={14} />
            {:else}
              <Icon name="x-circle" size={14} />
            {/if}
            CUDA
          </span>
          <span
            class="req-item"
            class:ok={cudaTestFor('CUDNN_PATH')?.ok}
            class:bad={cudaTestFor('CUDNN_PATH') && !cudaTestFor('CUDNN_PATH')!.ok}
          >
            {#if cudaTestFor('CUDNN_PATH')?.ok}
              <Icon name="check-circle" size={14} />
            {:else}
              <Icon name="x-circle" size={14} />
            {/if}
            cuDNN
          </span>
        </div>

        <Collapsible bind:open={showPathInstructions}>
          <CollapsibleTrigger class="path-hints-trigger">
            <Icon name="chevron-down" size={12} class={showPathInstructions ? 'chev-open' : ''} />
            <span>{t('gpu.cudaPathInstructions')}</span>
          </CollapsibleTrigger>
          <CollapsibleContent>
            <ul class="cuda-env-hint">
              <li>{t('gpu.cudaHintCudaAuto')}</li>
              <li>
                {t('gpu.cudaHintCudnnManual')}
                <ul>
                  <li>{t('gpu.cudaHintCudnnManualSub')}</li>
                </ul>
              </li>
              <li>{t('gpu.cudaHintRestart')}</li>
            </ul>
          </CollapsibleContent>
        </Collapsible>

        <!-- DLC state machine -->
        <div class="cuda-modal-actions">
          {#if $pluginStatus?.active_in_current_session}
            <div class="plugin-active-badge">
              <Icon name="check-circle" size={14} />
              <span>{t('gpu.cudaActive')}</span>
            </div>
            <button class="cuda-modal-uninstall" onclick={uninstallPlugin}
              >{t('gpu.cudaUninstallPlugin')}</button
            >
          {:else if $pluginStatus?.installed && $pluginStatus?.compatible === false}
            <div class="plugin-error">{t('gpu.cudaIncompatible')}</div>
            <button class="cuda-modal-uninstall" onclick={uninstallPlugin}
              >{t('gpu.cudaUninstallPlugin')}</button
            >
          {:else if $pluginStatus?.installed}
            <button class="cuda-modal-primary" onclick={restartApp}
              >{t('gpu.cudaRestartToActivate')}</button
            >
          {:else if $pluginDownload.active}
            <div class="plugin-progress">
              <div class="plugin-progress-bar">
                <div class="plugin-progress-fill" style="width: {$pluginDownload.progress}%"></div>
              </div>
              <div class="plugin-progress-meta">
                <span
                  >{$pluginDownload.status || t('gpu.cudaDownloading')}
                  {$pluginDownload.progress}%</span
                >
                <span
                  >{$pluginDownload.speed}{$pluginDownload.remaining
                    ? ` - ${t('gpu.cudaTimeRemaining', { time: $pluginDownload.remaining })}`
                    : ''}</span
                >
              </div>
            </div>
            <button class="cuda-modal-cancel" onclick={cancelPluginInstall}
              >{t('gpu.cudaCancelDownload')}</button
            >
          {:else}
            <button
              class="cuda-modal-primary"
              disabled={!allRequirementsMet}
              onclick={installPlugin}
            >
              {t('gpu.cudaDownloadPlugin')}
            </button>
            {#if !allRequirementsMet}
              <div class="requirements-unmet">{t('gpu.cudaRequirementsUnmet')}</div>
            {/if}
          {/if}
        </div>

        {#if $pluginError}
          <div class="plugin-error">{$pluginError}</div>
        {/if}
      {:else}
        <div class="cuda-modal-note">{t('gpu.cudaLinuxHint')}</div>
      {/if}
    </div>
  </div>
{/if}

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

  .tooltip-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 0.875rem;
    height: 0.875rem;
    font-size: 0.5625rem;
    font-weight: 600;
    color: #71717a;
    background: rgba(113, 113, 122, 0.2);
    border-radius: 50%;
    cursor: help;
    transition: all 0.15s;
  }

  .tooltip-trigger:hover {
    color: #a1a1aa;
    background: rgba(113, 113, 122, 0.3);
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

  .status-badge.clickable {
    cursor: pointer;
    transition: background 0.15s;
  }

  .status-badge.clickable:hover {
    background: rgba(239, 68, 68, 0.25);
  }

  .cuda-modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .cuda-modal {
    position: relative;
    background: #1c1c1e;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.5rem;
    padding: 1.25rem;
    padding-top: 2.75rem;
    max-width: 28rem;
    width: 90%;
  }

  .cuda-modal-title {
    font-size: 0.8125rem;
    font-weight: 600;
    color: #e4e4e7;
    margin-bottom: 0.625rem;
  }

  .cuda-modal-body {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .cuda-link {
    display: block;
    width: 100%;
    padding: 0.4rem 0.625rem;
    font-size: 0.6875rem;
    color: #60a5fa;
    background: rgba(59, 130, 246, 0.08);
    border: 1px solid rgba(59, 130, 246, 0.15);
    border-radius: 0.25rem;
    text-align: left;
    cursor: pointer;
    transition: background 0.15s;
  }

  .cuda-link:hover {
    background: rgba(59, 130, 246, 0.15);
  }

  .cuda-cmd-block {
    position: relative;
    padding: 0.5rem 0.625rem;
    padding-right: 2rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.25rem;
  }

  .cuda-cmd {
    display: block;
    font-size: 0.625rem;
    font-family: monospace;
    color: #e4e4e7;
    line-height: 1.5;
    word-break: break-all;
    user-select: text;
    -webkit-user-select: text;
    -webkit-user-drag: none;
    cursor: text;
  }

  .cuda-copy-btn {
    position: absolute;
    top: 0.25rem;
    right: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.375rem;
    height: 1.375rem;
    padding: 0;
    color: #52525b;
    background: transparent;
    border: none;
    border-radius: 0.1875rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .cuda-copy-btn:hover {
    color: #a1a1aa;
    background: rgba(255, 255, 255, 0.08);
  }

  .cuda-copy-btn.copied {
    color: #4ade80;
  }

  .cuda-modal-note {
    margin-top: 0.375rem;
    font-size: 0.625rem;
    color: #71717a;
  }

  .cuda-links-row {
    display: flex;
    gap: 0.5rem;
  }

  .cuda-links-row .cuda-link {
    flex: 1;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
    text-align: center;
  }

  .cuda-env-block {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding: 0.5rem 0.625rem;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    color: #d4d4d8;
  }

  .cuda-env-row {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .cuda-env-name {
    display: inline-block;
    align-self: flex-start;
    padding: 0 0.25rem;
    background: rgba(165, 180, 252, 0.12);
    border-radius: 0.125rem;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 0.6875rem;
    font-weight: 600;
    color: #a5b4fc;
  }

  .cuda-env-value {
    display: block;
    padding: 0.125rem 0.25rem;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 0.125rem;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 0.625rem;
    color: #d4d4d8;
    white-space: nowrap;
    overflow-x: auto;
  }

  .cuda-env-hint {
    /* list-style on ul + padding-left to give the bullets room */
    list-style: disc outside;
    margin: 0.375rem 0 0 0;
    padding: 0.375rem 0 0 1.125rem;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 0.625rem;
    color: #a1a1aa;
    line-height: 1.5;
  }

  .cuda-env-hint li {
    margin: 0.125rem 0;
  }

  /* Color the bullet marker to match the text */
  .cuda-env-hint li::marker {
    color: #a1a1aa;
  }

  .cuda-modal-actions {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    margin-top: 0.75rem;
  }

  .cuda-modal-primary,
  .cuda-modal-uninstall,
  .cuda-modal-cancel {
    padding: 0.5rem 0.75rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: #e4e4e7;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.25rem;
    cursor: pointer;
    transition:
      background 0.15s,
      opacity 0.15s;
  }

  .cuda-modal-primary {
    background: rgba(59, 130, 246, 0.18);
    border-color: rgba(59, 130, 246, 0.35);
    color: #bfdbfe;
  }

  .cuda-modal-primary:hover:not(:disabled),
  .cuda-modal-uninstall:hover,
  .cuda-modal-cancel:hover {
    background: rgba(255, 255, 255, 0.14);
  }

  .cuda-modal-primary:hover:not(:disabled) {
    background: rgba(59, 130, 246, 0.28);
  }

  .cuda-modal-primary:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .cuda-modal-uninstall {
    font-size: 0.6875rem;
    opacity: 0.7;
  }

  .requirements-unmet {
    font-size: 0.625rem;
    color: #a1a1aa;
    font-style: italic;
    text-align: center;
  }

  /* Requirements checklist */
  .cuda-requirements {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.625rem;
    padding: 0.375rem 0.5rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 0.25rem;
    font-size: 0.6875rem;
  }

  .requirements-label {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    color: #a1a1aa;
  }

  .requirements-link {
    background: none;
    border: none;
    padding: 0;
    color: #93c5fd;
    text-decoration: underline;
    text-decoration-style: dotted;
    font-size: inherit;
    cursor: pointer;
  }

  .requirements-link:hover {
    color: #bfdbfe;
  }

  .req-item {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    color: #a1a1aa;
  }

  .req-item.ok {
    background: rgba(132, 204, 22, 0.12);
    color: #a3e635;
  }

  .req-item.bad {
    background: rgba(239, 68, 68, 0.1);
    color: #f87171;
  }

  /* Inline row: tiny refresh button sits flush-left next to the env var name pill. */
  .cuda-env-header {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .env-refresh {
    background: none;
    border: none;
    padding: 0.125rem;
    color: #a1a1aa;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 0.125rem;
    flex-shrink: 0;
    line-height: 0;
  }

  .env-refresh:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: #e4e4e7;
  }

  .env-refresh :global(.spinning) {
    animation: spin 0.8s linear infinite;
  }

  /* Collapsible "Path validation instructions" trigger */
  :global(.path-hints-trigger) {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    margin-top: 0.5rem;
    padding: 0.25rem 0;
    background: none;
    border: none;
    color: #a1a1aa;
    font-size: 0.6875rem;
    cursor: pointer;
  }

  :global(.path-hints-trigger:hover) {
    color: #e4e4e7;
  }

  :global(.path-hints-trigger .chev-open) {
    transform: rotate(180deg);
    transition: transform 0.15s;
  }

  /* Download progress bar */
  .plugin-progress {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .plugin-progress-bar {
    width: 100%;
    height: 0.5rem;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 0.25rem;
    overflow: hidden;
  }

  .plugin-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #3b82f6, #60a5fa);
    transition: width 0.2s;
  }

  .plugin-progress-meta {
    display: flex;
    justify-content: space-between;
    font-size: 0.625rem;
    color: #a1a1aa;
  }

  .plugin-active-badge {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.5rem 0.75rem;
    background: rgba(132, 204, 22, 0.12);
    border: 1px solid rgba(132, 204, 22, 0.25);
    border-radius: 0.25rem;
    color: #a3e635;
    font-size: 0.75rem;
    font-weight: 500;
    justify-content: center;
  }

  .plugin-error {
    margin-top: 0.5rem;
    padding: 0.375rem 0.5rem;
    background: rgba(239, 68, 68, 0.12);
    border-radius: 0.25rem;
    color: #f87171;
    font-size: 0.6875rem;
  }

  /* "Actual: ..." line shown under each expected env var after pressing Test Paths.
	 * Lime green when the actual value contains the expected NVIDIA install subpath,
	 * red when it's empty / pointing somewhere unexpected. */
  .cuda-env-actual {
    display: block;
    margin-top: 0.125rem;
    padding: 0.125rem 0.25rem;
    border-radius: 0.125rem;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 0.625rem;
    white-space: nowrap;
    overflow-x: auto;
  }

  .cuda-env-actual.ok {
    background: rgba(132, 204, 22, 0.12);
    color: #a3e635; /* lime-400 */
  }

  .cuda-env-actual.bad {
    background: rgba(239, 68, 68, 0.12);
    color: #f87171; /* red-400 */
  }

  .cuda-modal-close:hover {
    background: rgba(255, 255, 255, 0.12);
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
