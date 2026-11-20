<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { modelStore } from '$lib/stores/modelStore';
  import { transformModelStore } from '$lib/stores/transformModelStore';
  import { getModelById } from '$lib/types/models';
  import { useFadeOnIdle } from '$lib/composables/useFadeOnIdle';
  import { tauriSettings } from '$lib/composables/useTauriSettings';
  import { bottomCenterReady } from '$lib/composables/usePageLoadAnimation';
  import { pageStore } from '$lib/stores/pageStore';
  import { onDestroy } from 'svelte';
  import { t } from '$lib/locales';
  import { executionProviderStore } from '$lib/stores/executionProvider';
  import { demoSession, DEMO_MODEL_ID, DEMO_PROVIDER } from '$lib/stores/demo';
  import { customModelChanged, getCustomModelInfo } from '$lib/services/customModelService';

  type Props = {
    /** Callback when model selector is clicked */
    onOpenDownloadModal: () => void;
    /** Whether the model selector should glow (after download complete) */
    glowing?: boolean;
  };

  let { onOpenDownloadModal, glowing = false }: Props = $props();

  // Model state from the centralized store
  // The scripted demo (DebugPanel > Demo) mocks a ready Parakeet 0.6B on GPU.
  let whisperModelExists = $derived($demoSession || $modelStore.modelExists);
  let currentSelectedModel = $derived($demoSession ? DEMO_MODEL_ID : $modelStore.selectedModel);

  // Imported custom model's display name - the registry row is a generic
  // "Custom Model" placeholder, the user knows their model by ITS name.
  let customModelName = $state<string | null>(null);
  $effect(() => {
    $customModelChanged;
    if (currentSelectedModel !== 'custom') {
      customModelName = null;
      return;
    }
    void getCustomModelInfo().then(
      info => (customModelName = info?.name ?? null),
      () => (customModelName = null)
    );
  });

  // Reactive setting for model switcher
  let showModelSwitcher = $derived($tauriSettings?.show_model_switcher !== false);

  // Transform model state -3-state same DRY pattern as the voice "Select AI Model"
  // FTUE indicator: red (off), lime (on + ready), blue pulsing (on but model needs download).
  let transformEnabled = $derived($tauriSettings?.transform_enabled ?? false);
  let transformSelectedModel = $derived($tauriSettings?.transform_model || 't5-mini-grammar');
  let transformModelReady = $derived(
    !!$transformModelStore.downloadedModels[transformSelectedModel]
  );
  let transformNeedsSetup = $derived(transformEnabled && !transformModelReady);

  // Execution provider from centralized store
  let activeProvider = $derived($demoSession ? DEMO_PROVIDER : $executionProviderStore.active);

  // Short labels for display
  const providerShortLabels: Record<string, string> = {
    direct_ml: 'DML',
    cuda: 'CUDA',
    cpu: 'CPU',
    core_ml: 'ML',
  };

  // Full label keys for tooltips (localized)
  const providerFullLabelKeys: Record<string, string> = {
    direct_ml: 'modelSelector.gpuDirectml',
    cuda: 'modelSelector.gpuCuda',
    cpu: 'modelSelector.gpuCpu',
    core_ml: 'modelSelector.gpuCoreml',
  };

  function openGpuSettings(e: MouseEvent) {
    e.stopPropagation();
    pageStore.openSettings('gpu-acceleration');
  }

  function openTransformSettings(e: MouseEvent) {
    e.stopPropagation();
    pageStore.openSettings('transform-model');
  }

  // Fade on idle behavior - extract stores
  const fade = useFadeOnIdle({ delay: 5000 });
  const fading = fade.fading;
  const hovered = fade.hovered;

  // Start fade when component mounts and model exists
  $effect(() => {
    if (whisperModelExists) {
      fade.startFade();
    }
  });

  onDestroy(() => fade.cleanup());
</script>

<div
  class="fixed bottom-4 left-1/2 -translate-x-1/2 transition-opacity duration-500 ease-out-quint {$bottomCenterReady
    ? 'opacity-100'
    : 'opacity-0'}"
>
  {#if showModelSwitcher}
    <!-- Powerline-style chained button bar -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="flex items-center bg-zinc-900/60 backdrop-blur-md rounded-lg border border-zinc-700/30 text-xs shadow-lg shadow-black/20 overflow-hidden transition-opacity duration-300 ease-out {$fading &&
      !$hovered
        ? 'opacity-40'
        : 'opacity-100'} {glowing ? 'model-selector-glow border-lime-500/50' : ''}"
      onmouseenter={fade.handleEnter}
      onmouseleave={fade.handleLeave}
    >
      <!-- Model Segment -->
      <button
        class="flex items-center gap-2 px-3 py-2 text-zinc-400 whitespace-nowrap transition-colors duration-150 hover:bg-zinc-800/80 hover:text-zinc-200 cursor-pointer"
        onclick={onOpenDownloadModal}
        title={t('modelSelector.changeModel')}
      >
        {#if whisperModelExists}
          <span class="w-1.5 h-1.5 rounded-full bg-lime-400 shadow-sm shadow-lime-400/50"></span>
          <span>
            {#if currentSelectedModel === 'custom'}
              <!-- Before the registry lookup: 'custom' IS a registry id, but its
                   registry name is a generic placeholder, not the user's model -->
              {customModelName ?? t('modelSelector.customModel')}
            {:else if getModelById(currentSelectedModel)}
              {getModelById(currentSelectedModel)?.name}
            {:else}
              {currentSelectedModel}
            {/if}
          </span>
        {:else}
          <span class="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse"></span>
          <span>{t('modelSelector.selectAiModel')}</span>
        {/if}
      </button>

      <!-- Separator -->
      <span class="text-zinc-700"></span>

      <!-- Execution Provider Segment -->
      {#if activeProvider && ($demoSession || !$executionProviderStore.isLoading)}
        <button
          class="px-3 py-2 text-[10px] font-medium transition-colors duration-150 hover:bg-zinc-800/80 cursor-pointer {activeProvider ===
          'cpu'
            ? 'text-amber-400'
            : 'text-zinc-400 hover:text-zinc-200'}"
          onclick={openGpuSettings}
          title={providerFullLabelKeys[activeProvider]
            ? t(providerFullLabelKeys[activeProvider])
            : activeProvider}
        >
          {providerShortLabels[activeProvider] || activeProvider.toUpperCase()}
        </button>
      {/if}

      <!-- Separator -->
      <span class="text-zinc-700"></span>

      <!-- Transform Segment -3-state: yellow = on but model not downloaded
           (warning, action needed via tooltip + click), lime = on + ready,
           red = off. Deviates from voice's blue-pulsing FTUE intentionally
           - yellow reads as "warning" rather than "first-time setup". -->
      <button
        class="px-3 py-2 text-[10px] font-bold transition-colors duration-150 hover:bg-zinc-800/80 cursor-pointer {transformNeedsSetup
          ? 'text-yellow-400'
          : transformEnabled
            ? 'text-lime-400'
            : 'text-red-400'}"
        onclick={openTransformSettings}
        title={transformNeedsSetup
          ? t('modelSelector.transformSetupNeeded')
          : transformEnabled
            ? t('modelSelector.transformEnabled')
            : t('modelSelector.transformDisabled')}
      >
        T
      </button>
    </div>
  {/if}
</div>
