<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Custom Model Card
   *
   * Shared card for the imported BYO model - used by both the Settings >
   * Custom Model section and the Import Custom Model modal's manage view.
   * Shows name, format/size badges, Active state, and Use/Replace/Remove.
   */

  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';
  import { cmdOpenDirectory } from '$lib/ipc/commands';
  import { platformCache } from '$lib/stores/platformCache';
  import type { CustomModelInfo } from '$lib/types/models';

  interface Props {
    info: CustomModelInfo;
    /** Managed directory of this model - target of "reveal in file manager". */
    path: string;
    /** Whether the custom model is the currently selected AI model. */
    active: boolean;
    /** Shown when not active - selects the custom model. */
    onUse?: () => void;
    onRemove: () => void;
  }

  let { info, path, active, onUse, onRemove }: Props = $props();

  // OS-appropriate label: Finder on macOS, File Explorer on Windows/WSL
  // (WSL reveals via explorer.exe), generic "file manager" elsewhere.
  const revealLabel =
    platformCache.platform === 'macos'
      ? t('customModel.revealFinder')
      : platformCache.platform === 'linux'
        ? t('customModel.revealFiles')
        : t('customModel.revealExplorer');

  async function reveal(): Promise<void> {
    try {
      await cmdOpenDirectory(path);
    } catch (e) {
      console.error('[byo] Failed to reveal model directory:', e);
    }
  }

  export function formatSize(bytes: number): string {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unit = 0;
    while (size >= 1024 && unit < units.length - 1) {
      size /= 1024;
      unit++;
    }
    return `${size.toFixed(1)} ${units[unit]}`;
  }
</script>

<div class="model-card">
  <div class="model-card-main">
    <span class="model-name" title={info.originalFilename}>{info.name}</span>
    <div class="badge-row">
      <span class="badge format-badge">{info.format.toUpperCase()}</span>
      <span class="badge">{formatSize(info.sizeBytes)}</span>
      {#if active}
        <span class="badge active-badge">{t('customModel.active')}</span>
      {/if}
    </div>
    {#if info.source !== 'file'}
      <span class="source-text">{info.source}</span>
    {/if}
  </div>
  <div class="card-actions">
    {#if !active && onUse}
      <button class="action-button primary" onclick={onUse}>
        {t('customModel.useThisModel')}
      </button>
    {/if}
    <button
      class="action-button icon-button"
      title={revealLabel}
      aria-label={revealLabel}
      onclick={reveal}
    >
      <Icon name="folder" size={13} />
    </button>
    <button class="action-button danger" onclick={onRemove}>
      {t('customModel.remove')}
    </button>
  </div>
</div>

<style>
  .model-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.625rem 0.75rem;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 0.375rem;
  }

  .model-card-main {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  .model-name {
    font-size: 0.8125rem;
    color: #e4e4e7;
    word-break: break-all;
    /* Tooltip shows the underlying model file name on hover. */
    cursor: help;
    width: fit-content;
  }

  .badge-row {
    display: flex;
    gap: 0.375rem;
  }

  .badge {
    padding: 0.0625rem 0.375rem;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 999px;
    font-size: 0.5625rem;
    color: #a1a1aa;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .format-badge {
    background: rgba(139, 92, 246, 0.2);
    color: #c4b5fd;
  }

  .active-badge {
    background: rgba(34, 197, 94, 0.15);
    color: #86efac;
  }

  .source-text {
    font-size: 0.625rem;
    color: #52525b;
    word-break: break-all;
  }

  .card-actions {
    display: flex;
    gap: 0.375rem;
    flex-shrink: 0;
  }

  .action-button {
    padding: 0.3125rem 0.625rem;
    background: rgba(255, 255, 255, 0.06);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    color: #d4d4d8;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .action-button:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .action-button.primary {
    background: rgba(34, 197, 94, 0.15);
    color: #86efac;
  }

  .action-button.primary:hover {
    background: rgba(34, 197, 94, 0.25);
  }

  .action-button.icon-button {
    display: flex;
    align-items: center;
    padding: 0.3125rem 0.4375rem;
    color: #a1a1aa;
  }

  .action-button.icon-button:hover {
    color: #d4d4d8;
  }

  .action-button.danger {
    color: #f87171;
  }

  .action-button.danger:hover {
    background: rgba(248, 113, 113, 0.12);
  }
</style>
