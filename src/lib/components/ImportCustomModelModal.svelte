<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Import Custom Model Modal
   *
   * Tabbed at top: Transcribe (fully functional) | Transform (coming-soon
   * teaser in v1). Transcribe imports a BYO ONNX model from a local file
   * (browse or drag-drop) or a HuggingFace repo URL, with progress and
   * validation states driven by the Rust custom_model commands.
   */
  import { onMount } from 'svelte';
  import ModalWindow from './ModalWindow.svelte';
  import ModalCloseButton from './ModalCloseButton.svelte';
  import ModalBackButton from './ModalBackButton.svelte';
  import ConfirmModal from './ConfirmModal.svelte';
  import CustomModelCard from './CustomModelCard.svelte';
  import { HintTooltip } from './settings/primitives';
  import { Icon } from '$lib/icons';
  import { t, translate } from '$lib/locales';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import { mustInvoke } from '$lib/ipc/invoke';
  import { modelStore } from '$lib/stores/modelStore';
  import { modelDownloadEvents } from '$lib/composables/useModelDownloadEvents';
  import {
    importCustomModelFromFile,
    importCustomModelFromUrl,
    listCustomModels,
    listHfOnnxFiles,
    parseCustomModelError,
    removeCustomModel,
    selectCustomModel,
  } from '$lib/services/customModelService';
  import { useCustomModelValidator } from '$lib/composables/useCustomModelValidator';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { docsUrl } from '$lib/utils/docsUrl';
  import { describeOnnxVariant } from '$lib/utils/onnxVariantHints';
  import { platformCache } from '$lib/stores/platformCache';
  import type { CustomModelEntry, CustomModelInfo, HfOnnxCandidate } from '$lib/types/models';

  interface Props {
    isOpen: boolean;
    onClose: () => void;
    /** Called after a successful import so the settings section refreshes. */
    onImported: (info: CustomModelInfo) => void;
  }

  let { isOpen = $bindable(), onClose, onImported }: Props = $props();

  const BROWSE_MODELS_URL =
    'https://huggingface.co/models?pipeline_tag=automatic-speech-recognition&library=onnx';

  const validator = useCustomModelValidator();

  // WSLg windows are foreign surfaces to Windows Explorer - OLE drag-drop
  // can never reach them, so the drop zone is misleading there. Show a plain
  // Browse button with the standard penguin hint instead.
  const isWsl = platformCache.isWsl;

  type Tab = 'transcribe' | 'transform';
  type Phase = 'idle' | 'listing' | 'downloading' | 'validating';
  /** Entry screen picks one path; there isn't room to show both the drop
   * zone and the HF form at once, so the form only reveals after a choice. */
  type ImportMode = 'choose' | 'local' | 'hf';

  let activeTab = $state<Tab>('transcribe');
  let importMode = $state<ImportMode>('choose');
  let phase = $state<Phase>('idle');
  let errorText = $state<string | null>(null);
  let selectedPath = $state<string | null>(null);
  let companionPath = $state<string | null>(null);
  let displayName = $state('');
  let hfUrl = $state('');
  let dragActive = $state(false);
  // Set when a bare repo URL resolves to more than one .onnx file - shows a
  // picker instead of importing immediately.
  let hfCandidates = $state<HfOnnxCandidate[]>([]);

  // Manage view: the modal doubles as the management surface for ALL imported
  // models. `replacing` flips to the import form ("Import another" / Back).
  let modelEntries = $state<CustomModelEntry[]>([]);
  let selectedModelPath = $state<string | null>(null);
  let replacing = $state(false);
  let pendingRemove = $state<CustomModelEntry | null>(null);
  let removeError = $state<string | null>(null);
  let compatExpanded = $state(true);

  const busy = $derived(phase !== 'idle');
  const showManageView = $derived(modelEntries.length > 0 && !replacing);
  const showImportFooter = $derived(!showManageView);

  async function refreshEntries() {
    try {
      modelEntries = await listCustomModels();
      const settings = await mustInvoke<{ custom_model_path?: string | null }>('get_settings');
      selectedModelPath = settings.custom_model_path ?? null;
    } catch {
      modelEntries = [];
    }
  }

  // If this modal is reopened while a download it started is still running
  // (eg via Settings, not the mini pill), redirect to the shared dialog that
  // actually owns that view instead of showing a stale/blank import form -
  // mirrors the handoff downloadHfFile already does when the download starts.
  $effect(() => {
    if (isOpen && phase === 'downloading') {
      modelDownloadEvents.openModal();
      onClose();
    }
  });

  // On open: load the imported models (drives manage vs import view).
  $effect(() => {
    if (!isOpen) return;
    void refreshEntries();
  });

  /** Select one imported model as active, via the same path the picker uses. */
  async function useImportedModel(entry: CustomModelEntry) {
    try {
      await selectCustomModel(entry.path);
      selectedModelPath = entry.path;
      await modelStore.selectModel('custom');
    } catch (error) {
      console.error('Failed to select custom model:', error);
    }
  }

  /** Optimistic remove: drop the row now, delete in the background. */
  function confirmRemove() {
    const entry = pendingRemove;
    pendingRemove = null;
    if (!entry) return;
    removeError = null;
    const previous = modelEntries;
    modelEntries = modelEntries.filter(e => e.path !== entry.path);
    if (selectedModelPath === entry.path) selectedModelPath = null;
    void removeCustomModel(entry.path)
      .then(refreshEntries)
      .catch(error => {
        console.error('Failed to remove custom model:', error);
        modelEntries = previous;
        removeError = translate('customModel.removeError');
      });
  }

  function reset() {
    phase = 'idle';
    importMode = 'choose';
    errorText = null;
    selectedPath = null;
    companionPath = null;
    displayName = '';
    hfUrl = '';
    hfCandidates = [];
    replacing = false;
  }

  /** Post-import: the new model becomes the active one, then the modal closes.
   * The backend already pointed settings at the fresh import, so only the
   * model id needs selecting here. onImported is the single close path -
   * calling onClose too would double-fire pageStore's close (and its
   * returnTo navigation). */
  async function finishImport(info: CustomModelInfo) {
    try {
      await modelStore.selectModel('custom');
    } catch (error) {
      console.error('Failed to select custom model:', error);
    }
    await refreshEntries().catch(() => {});
    reset();
    onImported(info);
  }

  // 'listing' and 'validating' are short, uncancellable operations with no
  // persisted state to resume - closing mid-flight would silently abandon
  // them with no way to reattach, so still block those. 'downloading' now
  // has a resumable, cancellable backend operation (custom_model.rs
  // import_from_hf preserves the .partial file on cancel), so it should
  // minimize instead of blocking - matching ModelDownloadProgress.svelte's
  // close button, which has no such guard at all.
  const blocksClose = $derived(phase === 'listing' || phase === 'validating');

  /** Whether the top-left back arrow has anywhere to go - if not, there's
   * no "previous layer" and only the X (hard close) makes sense. */
  const canGoBack = $derived(hfCandidates.length > 0 || replacing || importMode !== 'choose');

  /** What the back arrow's tooltip says it goes back to - hover should
   * never leave you guessing. */
  const backTargetLabel = $derived(
    hfCandidates.length > 0
      ? t('customModel.backTargetSearch')
      : importMode !== 'choose'
        ? t('customModel.backTargetChoice')
        : t('customModel.backTargetModels')
  );

  /** ESC / backdrop-click: back one level if a layer exists (matching the
   * visible back arrow), otherwise close outright - never both a "back"
   * and a "cancel" affordance fighting for the same gesture. Exported for
   * the global ESC handler (keyboardService.ts) to call via bind:this. */
  export function close() {
    if (blocksClose) return;
    if (phase === 'downloading') {
      // Minimize, not cancel: the download keeps running server-side.
      // modelDownloadEvents already treats model_id === 'custom' as
      // isDownloadActive without forcing isModalOpen
      // (useModelDownloadEvents.ts), so MiniDownloadProgress picks it
      // up once this modal is hidden. Must NOT reset()/cancelDownload()
      // here - this component stays mounted (see +page.svelte, no
      // {#if}), so `phase` and the shared progress store both keep
      // updating in the background and reappear correctly on reopen.
      onClose();
      return;
    }
    if (canGoBack) {
      goBack();
      return;
    }
    reset();
    onClose();
  }

  /** The top-right X: always a full close, never a step back - the arrow
   * owns "back," the X owns "cancel," per the user's explicit split. */
  function hardClose() {
    if (blocksClose) return;
    if (phase === 'downloading') {
      onClose();
      return;
    }
    reset();
    onClose();
  }

  function fileNameOf(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function acceptDroppedPath(path: string) {
    if (busy) return;
    if (path.toLowerCase().endsWith('.onnx.data')) {
      companionPath = path;
      return;
    }
    const result = validator.validatePath(path);
    if (!result.isValid) {
      errorText = result.error ?? null;
      return;
    }
    errorText = null;
    selectedPath = path;
    if (!displayName) {
      displayName = fileNameOf(path).replace(/\.onnx$/i, '');
    }
  }

  async function browseModelFile() {
    const picked = await openFileDialog({
      multiple: false,
      filters: [{ name: 'ONNX model', extensions: ['onnx'] }],
    });
    if (typeof picked === 'string') acceptDroppedPath(picked);
  }

  async function browseCompanionFile() {
    const picked = await openFileDialog({
      multiple: false,
      filters: [{ name: 'ONNX external data', extensions: ['data'] }],
    });
    if (typeof picked === 'string') companionPath = picked;
  }

  async function importFromFile() {
    if (!selectedPath || busy) return;
    errorText = null;
    phase = 'validating';
    try {
      const info = await importCustomModelFromFile(
        selectedPath,
        displayName.trim() || fileNameOf(selectedPath).replace(/\.onnx$/i, ''),
        companionPath ?? undefined
      );
      await finishImport(info);
    } catch (error) {
      const parsed = parseCustomModelError(error);
      errorText = translate(`customModel.errors.${parsed.code}`);
      phase = 'idle';
    }
  }

  /** Resolve the pasted repo/URL to .onnx candidates. A direct file link or a
   * repo with exactly one .onnx file downloads immediately; a repo with
   * several shows a picker instead of erroring. */
  async function importFromHf() {
    const input = hfUrl.trim();
    if (!input || busy) return;
    if (!validator.looksLikeHfRepo(input)) {
      errorText = translate('customModel.errors.HfRepoNotFound');
      return;
    }
    errorText = null;
    hfCandidates = [];
    phase = 'listing';
    try {
      const candidates = await listHfOnnxFiles(input);
      if (candidates.length === 1) {
        await downloadHfFile(input, candidates[0]);
      } else {
        hfCandidates = candidates;
        phase = 'idle';
      }
    } catch (error) {
      const parsed = parseCustomModelError(error);
      phase = 'idle';
      errorText = translate(`customModel.errors.${parsed.code}`);
    }
  }

  /** Download one resolved .onnx candidate, whether it was the sole match or
   * the user's pick from the list. */
  async function downloadHfFile(url: string, candidate: HfOnnxCandidate) {
    hfCandidates = [];
    phase = 'downloading';
    // Hand off to the SAME shared download dialog registry models use
    // (ModelDownloadProgress.svelte, mounted once at the page level) -
    // not a lookalike view in this modal. Synchronous so there's no flash
    // of neither dialog before the backend's model-download-start event
    // confirms the same state a moment later.
    modelDownloadEvents.beginCustomDownload({
      name: hfCandidateName(candidate.path),
      totalBytes: candidate.sizeBytes,
      hfUrl: url,
      hfFilePath: candidate.path,
    });
    onClose();
    try {
      const info = await importCustomModelFromUrl(url, candidate.path);
      await finishImport(info);
    } catch (error) {
      const parsed = parseCustomModelError(error);
      phase = 'idle';
      // User-initiated cancel is not an error - just return to the form.
      if (parsed.message.toLowerCase().includes('cancelled')) {
        errorText = null;
        return;
      }
      errorText = translate(`customModel.errors.${parsed.code}`);
    }
  }

  function cancelHfChoice() {
    hfCandidates = [];
  }

  /** Back to the "Browse Local File / Download from HuggingFace" entry screen. */
  function backToChooseImportMode() {
    importMode = 'choose';
    selectedPath = null;
    companionPath = null;
    displayName = '';
    hfUrl = '';
    hfCandidates = [];
    errorText = null;
  }

  /** One explicit step back - candidates -> URL input -> choose screen ->
   * models list - never more than one level, never both a "back" and a
   * "cancel" button competing for the same action. */
  function goBack() {
    if (hfCandidates.length > 0) {
      cancelHfChoice();
    } else if (importMode !== 'choose') {
      backToChooseImportMode();
    } else {
      replacing = false;
    }
  }

  function hfCandidateName(path: string): string {
    return path.split('/').pop() ?? path;
  }

  function formatBytes(bytes: number): string {
    if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(1)} GB`;
    if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  /** Clear transient import inputs (keeps the model list). Used entering the
   * import form so a previous failed attempt's file/URL/error doesn't reappear. */
  function resetImportFields() {
    importMode = 'choose';
    errorText = null;
    selectedPath = null;
    companionPath = null;
    displayName = '';
    hfUrl = '';
    hfCandidates = [];
  }

  onMount(() => {
    let unlistenDrop: (() => void) | undefined;

    (async () => {
      // Native drag-drop: HTML5 drop events in Tauri webviews don't carry
      // filesystem paths, so listen at the webview layer instead. Not
      // registered on WSL, where cross-OS drops can never arrive (a plain
      // Browse button is shown there instead).
      if (!isWsl) {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        unlistenDrop = await getCurrentWebview().onDragDropEvent(event => {
          if (!isOpen || activeTab !== 'transcribe') return;
          if (event.payload.type === 'over') {
            dragActive = true;
          } else if (event.payload.type === 'drop') {
            dragActive = false;
            for (const path of event.payload.paths) acceptDroppedPath(path);
          } else {
            dragActive = false;
          }
        });
      }
    })();

    return () => {
      unlistenDrop?.();
    };
  });
</script>

<!-- fadeMs 0: this modal takes over from the model picker in the same frame -
     any fade shows the screen behind (or a darker stacked backdrop) mid-handoff -->
<ModalWindow {isOpen} maxWidth="max-w-lg" fadeMs={0} onClose={close} hideCloseButton>
  {#if canGoBack}
    <ModalBackButton
      onClick={goBack}
      label={t('customModel.backToLabel', { target: backTargetLabel })}
    />
  {/if}
  <ModalCloseButton onClose={hardClose} />
  <div class="modal-body">
    <div class="header-row">
      <h2 class="modal-title">{t('customModel.importTitle')}</h2>
      <!-- launchUrl, not a bare anchor: target=_blank does nothing inside the Tauri webview -->
      <button
        class="docs-link"
        type="button"
        title={t('customModel.docsTooltip')}
        aria-label={t('customModel.docsTooltip')}
        onclick={() => launchUrl(docsUrl('/features/custom-models'))}
      >
        <Icon name="book" size={16} />
      </button>
    </div>

    <!-- Tabs -->
    <div class="tab-row" role="tablist">
      <button
        class="tab"
        class:active={activeTab === 'transcribe'}
        role="tab"
        aria-selected={activeTab === 'transcribe'}
        onclick={() => (activeTab = 'transcribe')}
      >
        {t('customModel.tabTranscribe')}
      </button>
      <button
        class="tab"
        class:active={activeTab === 'transform'}
        role="tab"
        aria-selected={activeTab === 'transform'}
        onclick={() => (activeTab = 'transform')}
      >
        {t('customModel.tabTransform')}
      </button>
    </div>

    <!-- tab-panel keeps a stable min-height so switching tabs never resizes the modal -->
    {#if activeTab === 'transform'}
      <div class="tab-panel coming-soon-panel">
        <Icon name="sparkles" size={20} />
        <p>{t('customModel.transformComingSoon')}</p>
      </div>
    {:else if showManageView}
      <!-- Manage view: every imported model, active state, Use / Remove
			     per model, plus a shared "Import another" action -->
      <div class="tab-panel">
        <div class="model-list" role="list">
          {#each modelEntries as entry (entry.path)}
            <CustomModelCard
              info={entry.info}
              path={entry.path}
              active={$modelStore.selectedModel === 'custom' && selectedModelPath === entry.path}
              onUse={() => useImportedModel(entry)}
              onRemove={() => (pendingRemove = entry)}
            />
          {/each}
        </div>
        {#if removeError}
          <p class="error-text">{removeError}</p>
        {/if}
        <div class="manage-actions">
          <button
            class="primary-button import-another-button"
            onclick={() => {
              resetImportFields();
              replacing = true;
            }}
          >
            {t('customModel.importAnother')}
          </button>
          {#if $modelStore.selectedModel === 'custom'}
            <button class="secondary-button done-button" onclick={close}>
              {t('customModel.done')}
            </button>
          {/if}
        </div>
      </div>
    {:else}
      <div class="tab-panel">
        <div class="scroll-region">
          {#if importMode === 'choose'}
            <!-- Entry screen: not enough room to show the drop zone and the HF
				     form at once, so pick a path first, then that form reveals. -->
            <div class="import-mode-choice">
              <button class="mode-choice-button" onclick={() => (importMode = 'local')}>
                <Icon name="upload" size={18} />
                <span>{t('customModel.browseLocalFile')}</span>
              </button>
              <button class="mode-choice-button" onclick={() => (importMode = 'hf')}>
                <Icon name="download" size={18} />
                <span>{t('customModel.downloadFromHf')}</span>
              </button>
            </div>
          {:else if importMode === 'local'}
            <!-- Local file import -->
            {#if isWsl}
              <!-- Windows Explorer cannot drag into WSLg windows - Browse only -->
              <div class="wsl-browse-row">
                <button
                  class="primary-button"
                  title={selectedPath ? fileNameOf(selectedPath) : undefined}
                  onclick={browseModelFile}
                  disabled={busy}
                >
                  {selectedPath ? fileNameOf(selectedPath) : t('customModel.browseButton')}
                </button>
                <span
                  class="linux-penguin"
                  title={translate('customModel.wslDropTooltip')}
                  aria-label={translate('customModel.wslDropTooltip')}
                >
                  <Icon name="brand-linux" size={14} />
                </span>
              </div>
              {#if companionPath}
                <span class="companion-name">+ {fileNameOf(companionPath)}</span>
              {/if}
            {:else}
              <div
                class="drop-zone"
                class:drag-active={dragActive}
                class:has-file={!!selectedPath}
                role="button"
                tabindex="0"
                onclick={browseModelFile}
                onkeydown={e => e.key === 'Enter' && browseModelFile()}
              >
                {#if selectedPath}
                  <span class="file-name">{fileNameOf(selectedPath)}</span>
                  {#if companionPath}
                    <span class="companion-name">+ {fileNameOf(companionPath)}</span>
                  {/if}
                {:else}
                  <Icon name="upload" size={20} />
                  <span>{t('customModel.dropHint')}</span>
                {/if}
              </div>
            {/if}

            <div class="secondary-row">
              <button
                class="link-button"
                title={t('customModel.companionTooltip')}
                onclick={browseCompanionFile}
                disabled={busy}
              >
                {companionPath ? t('customModel.companionAdded') : t('customModel.addCompanion')}
              </button>
            </div>

            {#if selectedPath}
              <div class="input-group">
                <label class="input-label" for="customModelDisplayName"
                  >{t('customModel.modelName')}</label
                >
                <input
                  id="customModelDisplayName"
                  type="text"
                  class="text-input"
                  placeholder={t('customModel.modelNamePlaceholder')}
                  bind:value={displayName}
                  disabled={busy}
                />
              </div>
              <button class="primary-button" onclick={importFromFile} disabled={busy}>
                {phase === 'validating'
                  ? t('customModel.validating')
                  : t('customModel.importModelButton')}
              </button>
            {/if}
          {:else if importMode === 'hf'}
            <!-- HuggingFace URL import -->
            {#if hfCandidates.length > 0}
              <p class="hint-text hf-choose-hint">{t('customModel.chooseHfFile')}</p>
              <div class="model-list hf-candidate-list" role="list">
                {#each hfCandidates as candidate (candidate.path)}
                  {@const variantHint = describeOnnxVariant(candidate.path)}
                  <button
                    class="hf-candidate-row"
                    onclick={() => downloadHfFile(hfUrl.trim(), candidate)}
                  >
                    <span class="hf-candidate-name-group">
                      <span class="hf-candidate-name">{hfCandidateName(candidate.path)}</span>
                      {#if variantHint}
                        <HintTooltip hint={variantHint} />
                      {/if}
                    </span>
                    <span class="hf-candidate-size">{formatBytes(candidate.sizeBytes)}</span>
                  </button>
                {/each}
              </div>
            {:else}
              <div class="hf-row">
                <input
                  type="text"
                  class="text-input hf-input"
                  placeholder={t('customModel.hfPlaceholder')}
                  bind:value={hfUrl}
                  disabled={busy}
                  onkeydown={e => e.key === 'Enter' && importFromHf()}
                />
                <button
                  class="primary-button"
                  onclick={importFromHf}
                  disabled={busy || !hfUrl.trim()}
                >
                  {phase === 'listing' ? t('customModel.checking') : t('customModel.importFromHf')}
                </button>
              </div>
            {/if}
          {/if}

          {#if phase === 'validating'}
            <p class="progress-text">{t('customModel.validating')}</p>
          {/if}

          {#if errorText}
            <p class="error-text">{errorText}</p>
          {/if}
        </div>

        {#if showImportFooter}
          <div class="compat-warning">
            <button
              type="button"
              class="compat-warning-header"
              onclick={() => (compatExpanded = !compatExpanded)}
              aria-expanded={compatExpanded}
            >
              <Icon name="warning" size={14} />
              <span>{t('customModel.compatWarningTitle')}</span>
              <Icon
                name="chevron-down"
                size={12}
                class="compat-chevron {compatExpanded ? 'open' : ''}"
              />
            </button>
            {#if compatExpanded}
              <div class="compat-columns">
                <div class="compat-col">
                  <div class="compat-row">
                    <Icon name="check" size={12} class="compat-icon-yes" />
                    <span>{t('customModel.compatWorks1')}</span>
                  </div>
                  <div class="compat-row">
                    <Icon name="check" size={12} class="compat-icon-yes" />
                    <span>{t('customModel.compatWorks2')}</span>
                  </div>
                  <div class="compat-row">
                    <Icon name="check" size={12} class="compat-icon-yes" />
                    <span>{t('customModel.compatWorks3')}</span>
                  </div>
                  <div class="compat-row">
                    <Icon name="check" size={12} class="compat-icon-yes" />
                    <span>{t('customModel.compatWorks4')}</span>
                  </div>
                </div>
                <div class="compat-col">
                  <div class="compat-row">
                    <Icon name="close" size={12} class="compat-icon-no" />
                    <span>{t('customModel.compatWont1')}</span>
                  </div>
                  <div class="compat-row">
                    <Icon name="close" size={12} class="compat-icon-no" />
                    <span>{t('customModel.compatWont2')}</span>
                  </div>
                  <div class="compat-row">
                    <Icon name="close" size={12} class="compat-icon-no" />
                    <span>{t('customModel.compatWont3')}</span>
                  </div>
                </div>
              </div>
            {/if}
          </div>
          <p class="hint-text modal-footer-link">
            {t('customModel.whereToFind')}
            <button
              class="hint-link"
              type="button"
              title="huggingface.co"
              onclick={() => launchUrl(BROWSE_MODELS_URL)}>{t('customModel.browseHf')}</button
            >
          </p>
        {/if}
      </div>
    {/if}
  </div>
</ModalWindow>

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
  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.25rem;
    /* Participate in ModalWindow's flex-column height chain so this can
		   shrink to fit within its 90vh cap instead of growing past it and
		   clipping the footer - see .scroll-region below. */
    flex: 1 1 auto;
    min-height: 0;
  }

  /* Scrolls internally when the candidate list / form is tall; the
	   compatibility footer below stays pinned and always visible. */
  .scroll-region {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding-right: 0.125rem;
  }

  .header-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .modal-title {
    font-size: 1rem;
    font-weight: 600;
    color: #e4e4e7;
    margin: 0;
  }

  .docs-link {
    display: inline-flex;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    color: #71717a;
    transition: color 0.15s ease;
  }

  .docs-link:hover {
    color: #d4d4d8;
  }

  .tab-row {
    display: flex;
    gap: 0.25rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .tab {
    padding: 0.375rem 0.75rem;
    font-size: 0.75rem;
    color: #71717a;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .tab:hover {
    color: #d4d4d8;
  }

  .tab.active {
    color: #e4e4e7;
    border-bottom-color: #8b5cf6;
  }

  /* Shared by both tab bodies: a FIXED height (not just a floor) so
	   switching tabs/views never resizes the modal. .scroll-region inside
	   the import form does the actual scrolling within this fixed box. */
  .tab-panel {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    height: 23rem;
    overflow: hidden;
  }

  /* Unlimited models: cap the visible height and scroll past ~4 cards. */
  .model-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-height: 15rem;
    overflow-y: auto;
    /* Breathing room so the scrollbar doesn't overlap card edges. */
    padding-right: 0.125rem;
  }

  .coming-soon-panel {
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 2rem 1rem;
    color: #71717a;
    font-size: 0.75rem;
    text-align: center;
  }

  .import-mode-choice {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    padding: 1rem 0;
  }

  .mode-choice-button {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding: 0.75rem 1rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.5rem;
    color: #d4d4d8;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition:
      border-color 0.15s ease,
      background 0.15s ease;
  }

  .mode-choice-button:hover {
    border-color: #8b5cf6;
    background: rgba(139, 92, 246, 0.06);
  }

  .drop-zone {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.375rem;
    min-height: 5.5rem;
    padding: 1rem;
    border: 1px dashed rgba(255, 255, 255, 0.15);
    border-radius: 0.375rem;
    color: #71717a;
    font-size: 0.75rem;
    cursor: pointer;
    transition:
      border-color 0.15s ease,
      background 0.15s ease;
  }

  .drop-zone:hover,
  .drop-zone.drag-active {
    border-color: #8b5cf6;
    background: rgba(139, 92, 246, 0.06);
  }

  .drop-zone.has-file {
    border-style: solid;
    color: #d4d4d8;
  }

  .file-name {
    font-size: 0.8125rem;
    color: #e4e4e7;
    word-break: break-all;
  }

  .companion-name {
    font-size: 0.6875rem;
    color: #71717a;
    word-break: break-all;
  }

  .secondary-row {
    display: flex;
    justify-content: flex-end;
  }

  .link-button {
    background: none;
    border: none;
    color: #71717a;
    font-size: 0.6875rem;
    cursor: pointer;
    text-decoration: underline;
  }

  .link-button:hover:not(:disabled) {
    color: #d4d4d8;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .input-label {
    font-size: 0.6875rem;
    color: #52525b;
  }

  .text-input {
    padding: 0.5rem 0.625rem;
    background: rgba(255, 255, 255, 0.04);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    color: #d4d4d8;
  }

  .text-input:focus {
    outline: none;
    background: rgba(255, 255, 255, 0.08);
  }

  .primary-button {
    padding: 0.5rem 0.875rem;
    background: #8b5cf6;
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: white;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .primary-button:hover:not(:disabled) {
    background: #7c3aed;
  }

  .primary-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .manage-actions {
    display: flex;
    gap: 0.5rem;
  }

  .import-another-button {
    flex: 2;
  }

  .done-button {
    flex: 1;
  }

  .secondary-button {
    padding: 0.5rem 0.875rem;
    background: rgba(255, 255, 255, 0.06);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: #d4d4d8;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .secondary-button:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .hf-row {
    display: flex;
    gap: 0.5rem;
  }

  .hf-input {
    flex: 1;
  }

  .hf-choose-hint {
    margin: 0;
  }

  .hf-candidate-list {
    max-height: 10rem;
  }

  .hf-candidate-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding: 0.5rem 0.625rem;
    background: rgba(255, 255, 255, 0.04);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    color: #d4d4d8;
    cursor: pointer;
    text-align: left;
    transition: background 0.15s ease;
  }

  .hf-candidate-row:hover {
    background: rgba(139, 92, 246, 0.12);
  }

  .hf-candidate-name-group {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    min-width: 0;
  }

  .hf-candidate-name {
    word-break: break-all;
  }

  .hf-candidate-size {
    flex-shrink: 0;
    color: #71717a;
    font-size: 0.6875rem;
  }

  .progress-text {
    font-size: 0.6875rem;
    color: #71717a;
    margin: 0;
  }

  .error-text {
    font-size: 0.75rem;
    color: #f87171;
    margin: 0;
  }

  .hint-text {
    font-size: 0.625rem;
    color: #52525b;
    margin: 0;
  }

  .wsl-browse-row {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .wsl-browse-row .primary-button {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Same inline penguin hint pattern as AudioInputSection */
  .linux-penguin {
    display: inline-flex;
    align-items: center;
    margin-left: 0.4em;
    line-height: 1;
    color: #a1a1aa;
    cursor: help;
    transition: transform 120ms ease;
  }

  .linux-penguin:hover {
    transform: scale(1.15);
  }

  .hint-link {
    color: #8b5cf6;
    background: none;
    border: none;
    padding: 0;
    font-size: inherit;
    cursor: pointer;
    text-decoration: underline;
  }

  .modal-footer-link {
    margin: 0;
  }

  /* Compatibility disclaimer - warning treatment matches the amber
	   partial-download banner in ModelDownloadProgress.svelte. */
  .compat-warning {
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.25);
    border-radius: 0.5rem;
    padding: 0.625rem 0.75rem;
    flex-shrink: 0;
  }

  .compat-warning-header {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    width: 100%;
    background: none;
    border: none;
    padding: 0;
    color: #fbbf24;
    font-size: 0.6875rem;
    font-weight: 600;
    cursor: pointer;
  }

  .compat-warning-header :global(.compat-chevron) {
    margin-left: auto;
    flex-shrink: 0;
    color: #fbbf24;
    transition:
      transform 0.2s ease,
      color 0.15s ease;
  }

  .compat-warning-header:hover :global(.compat-chevron) {
    color: #fde68a;
    stroke-width: 3;
  }

  .compat-warning-header :global(.compat-chevron.open) {
    transform: rotate(180deg);
  }

  .compat-columns {
    margin-top: 0.5rem;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
  }

  .compat-col {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .compat-row {
    display: flex;
    align-items: flex-start;
    gap: 0.375rem;
    font-size: 0.625rem;
    color: #a1a1aa;
    line-height: 1.4;
  }

  .compat-row span {
    /* Avoids orphan single-word lines like a lone "mel)" wrapping alone -
		   evens out line lengths instead of just breaking at the widest fit. */
    text-wrap: balance;
  }

  .compat-row :global(.compat-icon-yes) {
    color: #4ade80;
    flex-shrink: 0;
    margin-top: 0.125rem;
  }

  .compat-row :global(.compat-icon-no) {
    color: #f87171;
    flex-shrink: 0;
    margin-top: 0.125rem;
  }
</style>
