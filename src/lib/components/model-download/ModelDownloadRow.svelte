<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Single model row in the model-selection list.
   *
   * Renders the name, GPU hint, status icon (active radio / check / download arrow),
   * and handles click-to-select. Delegates compatibility logic to props.
   */

  import { t } from '$lib/locales';
  import type { ModelDefinition } from '$lib/types/models';
  import FeatureBadge from '$lib/components/settings/primitives/FeatureBadge.svelte';

  interface Props {
    model: ModelDefinition;
    isSelected: boolean;
    isDownloaded: boolean;
    isGpuRecommended: boolean;
    isIncompatible: boolean;
    gpuTooltip: string | null;
    onSelect: (modelId: string) => void;
  }

  let {
    model,
    isSelected,
    isDownloaded,
    isGpuRecommended,
    isIncompatible,
    gpuTooltip,
    onSelect,
  }: Props = $props();
</script>

<button
  onclick={() => onSelect(model.id)}
  class="model-card w-full px-4 py-2.5 rounded-xl text-left cursor-pointer
    {isSelected ? 'selected' : ''}
    {isIncompatible ? 'incompatible' : ''}"
>
  <div class="row-front flex items-center justify-between gap-3">
    <!-- Left: badges stacked above name so long names don't truncate -->
    <div class="min-w-0 flex-1">
      {#if isGpuRecommended || model.isRecommended || model.isAlpha}
        <div class="flex items-center gap-1 mb-0.5">
          {#if isGpuRecommended || model.isRecommended}
            <span class="badge-recommended">{t('model.recommended')}</span>
          {/if}
          {#if model.isAlpha}
            <FeatureBadge variant="alpha" />
          {/if}
        </div>
      {/if}
      <span class="block text-sm font-medium truncate text-zinc-100">
        {model.name}
        {#if model.languagesFull}
          <span class="badge-langs" title={model.languagesFull}>{model.languages}</span>
        {/if}
      </span>
    </div>

    <!-- Right: GPU recommendation text + status icon -->
    <div class="flex items-center gap-3 flex-shrink-0">
      {#if gpuTooltip}
        <span class="text-xs text-zinc-400 gpu-hint" title={gpuTooltip}
          >{model.gpuRecommendation}</span
        >
      {:else}
        <span class="text-xs text-zinc-400">{model.gpuRecommendation}</span>
      {/if}

      <!-- Status icons: distinct shapes for color-blind users -->
      {#if isSelected}
        <div
          class="w-6 h-6 rounded-full bg-lime-500/20 flex items-center justify-center"
          title={t('modelDownload.active') ?? 'Active'}
        >
          <svg class="w-4 h-4 text-lime-400" fill="currentColor" viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="5" />
          </svg>
        </div>
      {:else if isDownloaded}
        <div
          class="w-6 h-6 rounded-full bg-zinc-700/40 flex items-center justify-center"
          title={t('modelDownload.downloaded') ?? 'Downloaded'}
        >
          <svg
            class="w-4 h-4 text-zinc-400"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2.5"
          >
            <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
          </svg>
        </div>
      {:else}
        <div
          class="w-6 h-6 rounded-full bg-zinc-700/50 flex items-center justify-center"
          title={t('modelDownload.download') ?? 'Download'}
        >
          <svg
            class="w-4 h-4 text-zinc-500"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            stroke-width="2"
          >
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"
            />
          </svg>
        </div>
      {/if}
    </div>
  </div>
</button>

<style>
  .model-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
    transition:
      background-color 50ms ease,
      border-color 50ms ease;
  }

  .model-card:hover {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .model-card.incompatible {
    border-left: 2px solid var(--color-destructive);
  }

  .model-card.selected {
    background: rgba(132, 204, 22, 0.05);
    border-color: rgba(132, 204, 22, 0.2);
  }

  .badge-recommended {
    font-size: 0.5625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    background: rgba(132, 204, 22, 0.1);
    color: #84cc16;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .badge-langs {
    font-size: 0.5625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    background: rgba(96, 165, 250, 0.1);
    color: #60a5fa;
    margin-left: 0.375rem;
    vertical-align: middle;
    cursor: help;
    white-space: nowrap;
  }

  .badge-langs:hover {
    background: rgba(96, 165, 250, 0.2);
    color: #93c5fd;
  }

  .gpu-hint {
    text-decoration: underline;
    text-decoration-style: dashed;
    text-underline-offset: 2px;
    cursor: help;
    color: #a1a1aa;
  }

  .gpu-hint:hover {
    color: #d4d4d8;
  }
</style>
