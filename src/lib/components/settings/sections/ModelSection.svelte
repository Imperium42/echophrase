<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Model Section
   *
   * Voice model selection with minimal styling.
   * Shows CPU fallback badge when GPU acceleration unavailable.
   */

  import { onMount } from 'svelte';
  import { SettingsSection } from '../primitives';
  import { Icon } from '$lib/icons';
  import { cmdGetModelsDir, cmdOpenDirectory } from '$lib/ipc/commands';
  import { isWindows, isMac } from '$lib/utils/platform';
  import { getSettingsContext } from '../context';
  import { modelStore } from '$lib/stores/modelStore';
  import { loadGpuInfo, type GpuInfo } from '$lib/utils/GpuUtil';
  import { formatSize, getModelById } from '$lib/types/models';
  import type { ChineseScript } from '$lib/types/settings';
  import { availableLanguages } from '$lib/types/settings';
  import { t } from '$lib/locales';
  import { customModelChanged, getCustomModelInfo } from '$lib/services/customModelService';

  // Module-level cache: GPU hardware and execution-provider status are
  // effectively static for the lifetime of the process. Re-querying on every
  // Settings open fired two IPC calls each time the user opened the modal.
  let gpuInfoCache: GpuInfo | null = null;
  let cpuFallbackCache: boolean | null = null;

  interface Props {
    /** Callback to open model download dialog */
    onChangeModel: () => void;
  }

  let { onChangeModel }: Props = $props();

  const { isLoading, settings, updateSetting } = getSettingsContext();

  // Get selected model from modelStore
  const selectedModelValue = $derived($modelStore.selectedModel);
  const selectedModel = $derived(getModelById(selectedModelValue));

  // Imported custom model's display name - the registry/options label is a
  // generic "Custom Model" placeholder, the user knows their model by ITS name.
  let customModelName = $state<string | null>(null);
  $effect(() => {
    $customModelChanged;
    if (selectedModelValue !== 'custom') {
      customModelName = null;
      return;
    }
    void getCustomModelInfo().then(
      info => (customModelName = info?.name ?? null),
      () => (customModelName = null)
    );
  });

  // Chinese script conversion - only shown for models with Eastern language support
  const supportsEasternLanguages = $derived(
    getModelById(selectedModelValue)?.supportsEasternLanguages ?? false
  );
  const chineseScript = $derived(($settings?.chinese_script ?? 'traditional') as ChineseScript);
  const scriptOptions: { value: ChineseScript; label: string }[] = [
    { value: 'traditional', label: t('modelSettings.chineseScriptTraditional') },
    { value: 'simplified', label: t('modelSettings.chineseScriptSimplified') },
  ];

  function selectChineseScript(value: ChineseScript) {
    if (value !== chineseScript) {
      updateSetting('chinese_script', value);
    }
  }

  // Transcription language hint - only shown for models that consume it (Qwen3-ASR)
  const supportsLanguageHint = $derived(
    getModelById(selectedModelValue)?.supportsLanguageHint ?? false
  );
  const transcriptionLanguage = $derived($settings?.language ?? 'auto');

  function selectLanguage(value: string) {
    if (value !== transcriptionLanguage) {
      updateSetting('language', value);
    }
  }

  // GPU info
  let gpuInfo: GpuInfo = $state({ name: null, vramGb: null, vramMb: null, displayString: null });

  // Execution provider status for CPU fallback badge
  let isCpuFallback = $state(false);

  function scrollToGpuSection() {
    const section = document.getElementById('gpu-acceleration');
    section?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    section?.classList.add('section-highlight');
    setTimeout(() => section?.classList.remove('section-highlight'), 1500);
  }

  onMount(async () => {
    if (typeof window !== 'undefined' && (window as any).__TAURI_INTERNALS__) {
      if (gpuInfoCache && cpuFallbackCache !== null) {
        gpuInfo = gpuInfoCache;
        isCpuFallback = cpuFallbackCache;
        return;
      }
      const { invoke } = await import('@tauri-apps/api/core');
      gpuInfo = await loadGpuInfo();
      gpuInfoCache = gpuInfo;

      // Check if running in CPU fallback mode
      try {
        const epStatus = await invoke<{ is_cpu_fallback: boolean }>('get_execution_provider');
        isCpuFallback = epStatus.is_cpu_fallback;
        cpuFallbackCache = isCpuFallback;
      } catch {
        // Silent fail - command may not be available
        cpuFallbackCache = false;
      }
    }
  });

  // Tooltip names the platform's actual file manager. WSL routes through
  // explorer.exe but reports as Linux - the generic string is close enough
  // and avoids an extra IPC just to detect it.
  const openModelsDirLabel = isWindows()
    ? t('modelSettings.openModelsDirWindows')
    : isMac()
      ? t('modelSettings.openModelsDirMac')
      : t('modelSettings.openModelsDirOther');

  /** Reveal the downloaded-models folder in the OS file manager. */
  async function openModelsDir() {
    try {
      // Resolved lazily on click - resolving in onMount would cost an IPC
      // every time Settings opens, for a button most users never press.
      const dir = await cmdGetModelsDir();
      await cmdOpenDirectory(dir);
    } catch (err) {
      console.warn('[ModelSection] Failed to open models directory:', err);
    }
  }
</script>

{#snippet modelsDirButton()}
  <button
    class="models-dir-btn"
    title={openModelsDirLabel}
    aria-label={openModelsDirLabel}
    onclick={openModelsDir}
  >
    <Icon name="folder" size={12} />
  </button>
{/snippet}

<SettingsSection title={t('modelSettings.title')} icon="model" titleExtra={modelsDirButton}>
  {#if gpuInfo.displayString}
    <div class="gpu-info">{gpuInfo.displayString}</div>
  {/if}
  {#if $isLoading}
    <div class="loading-state">
      <div class="spinner"></div>
      <span>{t('modelSettings.loading')}</span>
    </div>
  {:else}
    <div class="model-row">
      <div class="model-info">
        {#if $modelStore.modelExists}
          <span class="model-name">
            {selectedModelValue === 'custom'
              ? (customModelName ?? t('modelSelector.customModel'))
              : selectedModel?.name || selectedModelValue}
            {#if isCpuFallback}
              <button
                class="cpu-badge"
                title={t('modelSettings.cpuTooltip')}
                onclick={scrollToGpuSection}>{t('modelSettings.cpuBadge')}</button
              >
            {/if}
          </span>
          {#if selectedModel && selectedModel.sizeBytes > 0 && selectedModel.id !== 'custom'}
            <span class="model-meta">{formatSize(selectedModel.sizeBytes)}</span>
          {/if}
        {:else}
          <span class="model-name placeholder">{t('modelSettings.noneSelected')}</span>
        {/if}
      </div>
      {#if $modelStore.modelExists}
        <button class="change-btn" onclick={onChangeModel}>{t('modelSettings.change')}</button>
      {:else}
        <button
          class="setup-btn"
          onclick={onChangeModel}
          title={t('modelSettings.setupRequiredTooltip')}>{t('modelSettings.setupBadge')}</button
        >
      {/if}
    </div>
    {#if supportsLanguageHint && $modelStore.modelExists}
      <div class="script-row">
        <span class="script-label" title={t('modelSettings.languageTooltip')}>
          {t('modelSettings.languageLabel')}
        </span>
        <div class="select-wrapper">
          <select
            class="select-field"
            value={transcriptionLanguage}
            onchange={e => selectLanguage(e.currentTarget.value)}
          >
            {#each availableLanguages as lang (lang.value)}
              <option value={lang.value}>
                {lang.value === 'auto' ? t('modelSettings.languageAuto') : lang.label}
              </option>
            {/each}
          </select>
          <svg class="select-arrow" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"
            ></path>
          </svg>
        </div>
      </div>
    {/if}
    {#if supportsEasternLanguages && $modelStore.modelExists}
      <div class="script-row">
        <span class="script-label" title={t('modelSettings.chineseScriptTooltip')}>
          {t('modelSettings.chineseScriptLabel')}
        </span>
        <div
          class="script-segments"
          role="radiogroup"
          aria-label={t('modelSettings.chineseScriptLabel')}
        >
          {#each scriptOptions as option (option.value)}
            <button
              class="script-segment"
              class:active={chineseScript === option.value}
              role="radio"
              aria-checked={chineseScript === option.value}
              onclick={() => selectChineseScript(option.value)}
            >
              {option.label}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</SettingsSection>

<style>
  /* Header-sized sibling of OutputSection's `.dir-btn` */
  .models-dir-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.25rem;
    height: 1.25rem;
    padding: 0;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .models-dir-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #a1a1aa;
  }

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

  .model-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
  }

  .model-info {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    flex: 1;
    min-width: 0;
  }

  .model-name {
    font-size: 0.8125rem;
    color: #d4d4d8;
  }

  .model-meta {
    font-size: 0.6875rem;
    color: #52525b;
  }

  .change-btn {
    padding: 0.375rem 0.625rem;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 450;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .change-btn:hover {
    background: rgba(255, 255, 255, 0.04);
    color: #a1a1aa;
  }

  .gpu-info {
    font-size: 0.6875rem;
    color: #52525b;
    margin-bottom: 0.5rem;
  }

  .cpu-badge {
    font-size: 0.5625rem;
    font-weight: 600;
    color: #eab308;
    background: rgba(234, 179, 8, 0.15);
    border: none;
    padding: 0.0625rem 0.25rem;
    border-radius: 0.1875rem;
    margin-left: 0.375rem;
    vertical-align: middle;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .cpu-badge:hover {
    background: rgba(234, 179, 8, 0.3);
    color: #facc15;
  }

  .setup-btn {
    padding: 0.375rem 0.75rem;
    background: rgba(234, 179, 8, 0.15);
    border: 1px solid rgba(234, 179, 8, 0.3);
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 600;
    color: #eab308;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease,
      border-color 0.15s ease;
  }

  .setup-btn:hover {
    background: rgba(234, 179, 8, 0.25);
    border-color: rgba(234, 179, 8, 0.5);
    color: #facc15;
  }

  .model-name.placeholder {
    color: #71717a;
    font-style: italic;
  }

  .script-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.375rem 0;
  }

  .script-label {
    font-size: 0.75rem;
    color: #a1a1aa;
    cursor: help;
  }

  .script-segments {
    display: inline-flex;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.375rem;
    padding: 0.125rem;
    gap: 0.125rem;
  }

  .script-segment {
    padding: 0.25rem 0.5rem;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .script-segment:hover {
    color: #a1a1aa;
  }

  .script-segment.active {
    background: rgba(255, 255, 255, 0.1);
    color: #e4e4e7;
  }

  .select-wrapper {
    position: relative;
    min-width: 140px;
  }

  .select-field {
    width: 100%;
    padding: 0.375rem 1.75rem 0.375rem 0.625rem;
    background: rgba(255, 255, 255, 0.04);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    color: #d4d4d8;
    cursor: pointer;
    appearance: none;
    transition: background 0.15s ease;
  }

  .select-field:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
  }

  .select-field:focus {
    outline: none;
    background: rgba(255, 255, 255, 0.08);
  }

  .select-arrow {
    position: absolute;
    right: 0.5rem;
    top: 50%;
    transform: translateY(-50%);
    width: 1rem;
    height: 1rem;
    color: #71717a;
    pointer-events: none;
  }
</style>
