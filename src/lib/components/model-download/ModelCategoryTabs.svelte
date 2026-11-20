<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Category tab switcher for the model download dialog.
   *
   * Renders filter checkboxes (hide incompatible, Eastern-only) above
   * the model rows. The actual model list is rendered by the parent using
   * ModelDownloadRow - this component only owns the filter controls.
   */

  import { t } from '$lib/locales';
  import type { ModelCategory } from '$lib/types/models';
  import ModelDownloadRow from './ModelDownloadRow.svelte';
  import { MODEL_REGISTRY, type ModelDefinition } from '$lib/types/models';

  interface Props {
    category: ModelCategory;
    filteredModels: ModelDefinition[];
    downloadedModels: Record<string, boolean>;
    selectedModel: string;
    hideIncompatible: boolean;
    easternOnly: boolean;
    hasVram: boolean;
    gpuDisplayString: string | null;
    onHideIncompatibleChange: (v: boolean) => void;
    onEasternOnlyChange: (v: boolean) => void;
    onSelectModel: (id: string) => void;
    isModelCompatible: (model: ModelDefinition) => boolean;
    isRecommendedForGpu: (id: string) => boolean;
    getGpuTooltip: (rec: string) => string | null;
  }

  let {
    category,
    filteredModels,
    downloadedModels,
    selectedModel,
    hideIncompatible,
    easternOnly,
    hasVram,
    gpuDisplayString,
    onHideIncompatibleChange,
    onEasternOnlyChange,
    onSelectModel,
    isModelCompatible,
    isRecommendedForGpu,
    getGpuTooltip,
  }: Props = $props();
</script>

<!-- min-h reserves space for ~2 cards + separator + coming-soon card so toggling filters
     doesn't collapse the modal body height -->
<div class="space-y-2 min-h-[280px] max-h-[400px] overflow-y-auto pr-1 model-list">
  <!-- Filter row: hide incompatible (left) + Eastern language (right) -->
  {#if hasVram || category === 'transcription'}
    <div class="filter-row">
      {#if hasVram}
        <label class="filter-checkbox">
          <input
            type="checkbox"
            checked={hideIncompatible}
            onchange={e => onHideIncompatibleChange(e.currentTarget.checked)}
          />
          <span>{t('modelDownload.hideIncompatible')}</span>
        </label>
      {/if}
      {#if category === 'transcription'}
        <label
          class="filter-checkbox filter-checkbox-right"
          title={t('modelDownload.easternLangFilterTooltip') ?? ''}
        >
          <input
            type="checkbox"
            checked={easternOnly}
            onchange={e => onEasternOnlyChange(e.currentTarget.checked)}
          />
          <span>{t('modelDownload.easternLangFilter')}</span>
        </label>
      {/if}
    </div>
  {/if}

  {#if easternOnly && category === 'transcription' && filteredModels.length === 0}
    <p class="text-xs text-zinc-500 px-1 py-2">{t('modelDownload.noEasternModels')}</p>
  {/if}

  {#each filteredModels as model (model.id)}
    {@const isDownloaded = !!downloadedModels[model.id]}
    {@const isSelected = selectedModel === model.id && isDownloaded}
    {@const isGpuRecommended = isRecommendedForGpu(model.id)}
    {@const isIncompatible = !isModelCompatible(model)}
    {@const gpuTooltip = getGpuTooltip(model.gpuRecommendation)}
    <ModelDownloadRow
      {model}
      {isSelected}
      {isDownloaded}
      {isGpuRecommended}
      {isIncompatible}
      {gpuTooltip}
      onSelect={onSelectModel}
    />
  {/each}

  <!-- Separator -->
  <div class="border-t border-zinc-700/50 my-3"></div>

  <!-- Coming-soon entries (same category only) -->
  {#each MODEL_REGISTRY.filter(m => m.isComingSoon && m.category === category) as model (model.id)}
    <div class="model-card-coming-soon w-full px-4 py-3 rounded-xl text-left">
      <div class="flex items-start justify-between gap-3">
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-2 flex-wrap mb-1">
            <span class="font-semibold text-zinc-500">{model.name}</span>
            <span class="badge-coming-soon">{t('badges.comingSoon')}</span>
          </div>
          <p class="text-xs text-zinc-600 mt-1">{model.description}</p>
        </div>
        <div class="flex-shrink-0 mt-1">
          <div class="w-6 h-6 rounded-full bg-zinc-800/50 flex items-center justify-center">
            <svg class="w-4 h-4 text-zinc-600" fill="currentColor" viewBox="0 0 24 24">
              <path
                fill-rule="evenodd"
                d="M12 1.5a5.25 5.25 0 00-5.25 5.25v3a3 3 0 00-3 3v6.75a3 3 0 003 3h10.5a3 3 0 003-3v-6.75a3 3 0 00-3-3v-3c0-2.9-2.35-5.25-5.25-5.25zm3.75 8.25v-3a3.75 3.75 0 10-7.5 0v3h7.5z"
                clip-rule="evenodd"
              />
            </svg>
          </div>
        </div>
      </div>
    </div>
  {/each}
</div>

{#if gpuDisplayString}
  <div class="gpu-recommendation-footer">
    {t('modelDownload.recommendingFor', { gpu: gpuDisplayString })}
  </div>
{/if}

<style>
  .model-list {
    cursor: default;
    user-select: none;
    padding-bottom: 1.25rem;
  }

  .model-list::-webkit-scrollbar {
    width: 4px;
  }
  .model-list::-webkit-scrollbar-track {
    background: transparent;
  }
  .model-list::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 2px;
  }
  .model-list::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .filter-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.25rem 0;
  }

  .filter-checkbox-right {
    margin-left: auto;
  }

  .filter-checkbox {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: #71717a;
    cursor: pointer;
    padding: 0.25rem 0;
    user-select: none;
  }

  .filter-checkbox input[type='checkbox'] {
    width: 0.875rem;
    height: 0.875rem;
    border-radius: 0.25rem;
    border: 1px solid #3f3f46;
    background: rgba(255, 255, 255, 0.02);
    cursor: pointer;
    accent-color: #84cc16;
  }

  .filter-checkbox:hover {
    color: #a1a1aa;
  }

  .model-card-coming-soon {
    opacity: 0.4;
    cursor: not-allowed;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
  }

  .badge-coming-soon {
    font-size: 0.5625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    background: rgba(113, 113, 122, 0.1);
    color: #71717a;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .gpu-recommendation-footer {
    position: absolute;
    bottom: 0.75rem;
    left: 0;
    right: 0;
    text-align: center;
    font-size: 0.625rem;
    color: #52525b;
    z-index: 10;
    pointer-events: none;
    letter-spacing: 0.01em;
  }
</style>
