<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * GPU Status Badge
   *
   * Minimal indicator showing current GPU acceleration status.
   * Uses centralized executionProviderStore (single source of truth).
   */

  import { executionProviderStore } from '$lib/stores/executionProvider';
  import { platformCache } from '$lib/stores/platformCache';

  interface Props {
    /** Badge size variant */
    size?: 'sm' | 'md';
  }

  let { size = 'sm' }: Props = $props();

  const isWsl = platformCache.isWsl;

  const providerConfig: Record<string, { label: string; color: string; tooltip: string }> = {
    cuda: {
      label: 'CUDA',
      color: 'text-lime-400',
      tooltip: 'NVIDIA CUDA - GPU acceleration active',
    },
    direct_ml: {
      label: 'DirectML',
      color: 'text-blue-400',
      tooltip: 'DirectML - Windows GPU acceleration',
    },
    core_ml: {
      label: 'CoreML',
      color: 'text-purple-400',
      tooltip: 'CoreML - Apple GPU acceleration',
    },
    cpu: {
      label: 'CPU',
      color: 'text-yellow-400',
      tooltip: 'CPU mode - No GPU acceleration',
    },
  };

  let provider = $derived($executionProviderStore.active);
  let config = $derived(providerConfig[provider] || providerConfig.cpu);
  let tooltip = $derived(
    provider === 'cpu' && isWsl ? 'CPU mode - CUDA not compiled into backend' : config.tooltip
  );
  let textSize = $derived(size === 'sm' ? 'text-[10px]' : 'text-xs');
</script>

{#if $executionProviderStore.isLoading}
  <span class="{textSize} text-zinc-500 font-medium">...</span>
{:else}
  <span class="{textSize} {config.color} font-medium cursor-default" title={tooltip}>
    {config.label}
  </span>
{/if}
