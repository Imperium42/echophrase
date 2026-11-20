<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Custom Models Section
   *
   * BYO ONNX transcription models - as many as the user wants. This section
   * is a compact summary, not the full list. Two states:
   * - nothing imported: "Import custom models" button opening
   *   ImportCustomModelModal (which shows the full list)
   * - imported: a single card for the active custom model (if one
   *   is selected), plus a "Manage custom models" button opening the same
   *   modal for the full list, add, use, and remove
   *
   * Removal (still available here for the active model) is optimistic: the
   * row disappears immediately, deletion runs in the background, and the
   * row comes back with an error if it fails.
   */

  import { onMount } from 'svelte';
  import { SettingsSection } from '../primitives';
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';
  import { pageStore, showImportCustomModelModal } from '$lib/stores/pageStore';
  import ConfirmModal from '../../ConfirmModal.svelte';
  import CustomModelCard from '../../CustomModelCard.svelte';
  import {
    customModelChanged,
    listCustomModels,
    removeCustomModel,
  } from '$lib/services/customModelService';
  import { modelStore } from '$lib/stores/modelStore';
  import { mustInvoke } from '$lib/ipc/invoke';
  import type { CustomModelEntry } from '$lib/types/models';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { docsUrl } from '$lib/utils/docsUrl';

  let entries = $state<CustomModelEntry[]>([]);
  let selectedPath = $state<string | null>(null);
  let pendingRemove = $state<CustomModelEntry | null>(null);
  let removeError = $state<string | null>(null);

  async function refresh() {
    try {
      entries = await listCustomModels();
      const settings = await mustInvoke<{ custom_model_path?: string | null }>('get_settings');
      selectedPath = settings.custom_model_path ?? null;
    } catch {
      entries = [];
    }
  }

  onMount(refresh);

  // Refresh the list whenever the import modal closes OR any surface
  // imports/selects/removes a model - keeps this section in sync even
  // while the modal is still open above it.
  $effect(() => {
    $customModelChanged;
    $showImportCustomModelModal;
    void refresh();
  });

  /** Optimistic remove: drop the row now, delete in the background. */
  function confirmRemove() {
    const entry = pendingRemove;
    pendingRemove = null;
    if (!entry) return;
    removeError = null;
    const previous = entries;
    entries = entries.filter(e => e.path !== entry.path);
    if (selectedPath === entry.path) selectedPath = null;
    void removeCustomModel(entry.path)
      .then(refresh)
      .catch(error => {
        console.error('Failed to remove custom model:', error);
        entries = previous;
        removeError = t('customModel.removeError');
      });
  }

  const activeEntry = $derived(
    $modelStore.selectedModel === 'custom'
      ? (entries.find(e => e.path === selectedPath) ?? null)
      : null
  );
</script>

<SettingsSection title={t('customModel.title')} icon="customModel">
  <div class="section-content">
    <div class="header-row">
      <p class="section-hint">{t('customModel.sectionHint')}</p>
      <!-- launchUrl, not a bare anchor: target=_blank does nothing inside the Tauri webview -->
      <button
        class="docs-link"
        type="button"
        title={t('customModel.docsTooltip')}
        aria-label={t('customModel.docsTooltip')}
        onclick={() => launchUrl(docsUrl('/features/custom-models'))}
      >
        <Icon name="book" size={14} />
      </button>
    </div>

    {#if activeEntry}
      <CustomModelCard
        info={activeEntry.info}
        path={activeEntry.path}
        active={true}
        onRemove={() => (pendingRemove = activeEntry)}
      />
    {/if}
    {#if removeError}
      <p class="error-text">{removeError}</p>
    {/if}
    <button class="import-button" onclick={() => pageStore.openImportCustomModelModal()}>
      <Icon name="upload" size={14} />
      {entries.length > 0 ? t('customModel.manageButton') : t('customModel.importButton')}
    </button>
  </div>
</SettingsSection>

<ConfirmModal
  isOpen={pendingRemove !== null}
  title={t('customModel.removeConfirmTitle')}
  message={t('customModel.removeConfirmBody')}
  confirmText={t('customModel.remove')}
  cancelText={t('common.cancel')}
  danger
  onConfirm={confirmRemove}
  onCancel={() => (pendingRemove = null)}
/>

<style>
  .section-content {
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
  }

  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .section-hint {
    font-size: 0.6875rem;
    color: #52525b;
    margin: 0;
  }

  .docs-link {
    display: inline-flex;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    color: #52525b;
    transition: color 0.15s ease;
  }

  .docs-link:hover {
    color: #d4d4d8;
  }

  .error-text {
    font-size: 0.6875rem;
    color: #f87171;
    margin: 0;
  }

  .import-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
    padding: 0.5rem 0.75rem;
    background: rgba(139, 92, 246, 0.15);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    color: #c4b5fd;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .import-button:hover,
  .import-button:hover {
    background: rgba(139, 92, 246, 0.25);
  }
</style>
