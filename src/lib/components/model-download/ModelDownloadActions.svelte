<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Cancel / Pause action group for an in-progress model download.
   */

  import { t } from '$lib/locales';

  interface Props {
    progress: number;
    onCancel: () => void;
  }

  let { progress, onCancel }: Props = $props();
</script>

<button onclick={onCancel} class="btn-secondary w-full">
  {progress >= 1 ? t('modelDownload.pause') : t('common.cancel')}
</button>

<style>
  /* Duplicated from ModelDownloadProgress.svelte's .btn-secondary - Svelte
     scopes <style> blocks per-component, so that rule never reaches this
     component's own markup even though the class name matches. If these
     drift again, extract a shared global class into app.css instead. */
  .btn-secondary {
    flex: 1;
    padding: 0.625rem 1rem;
    background: #27272a;
    color: #d4d4d8;
    border-radius: 0.5rem;
    font-weight: 500;
    font-size: 0.875rem;
    cursor: pointer;
    border: none;
    transition: background-color 50ms ease;
  }

  .btn-secondary:hover {
    background: #3f3f46;
  }

  .btn-secondary:active {
    background: #18181b;
  }
</style>
