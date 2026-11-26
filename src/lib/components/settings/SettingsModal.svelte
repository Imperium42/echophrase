<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Settings Modal
   *
   * Main settings container that provides context to all sections.
   * Uses ModalWindow as the base, sets up the settings context,
   * and composes all settings sections.
   */

  import { onMount, tick } from 'svelte';
  import ModalWindow from '../ModalWindow.svelte';
  import { modelStore } from '$lib/stores/modelStore';
  import { transformModelStore } from '$lib/stores/transformModelStore';
  import { modelDownloadEvents } from '$lib/composables/useModelDownloadEvents';
  import Skeleton from '../ui/Skeleton.svelte';
  import SettingsHeader from './SettingsHeader.svelte';
  import SettingsFooter from './SettingsFooter.svelte';
  import { devLog } from '$lib/utils/devLog';

  import {
    LanguageSection,
    AudioInputSection,
    ModelSection,
    CustomModelSection,
    GpuAccelerationSection,
    OptimizationSection,
    OutputSection,
    TransformModelSection,
    RecordingSection,
    HotkeysSection,
    CustomizeSection,
    McpServerSection,
    AccessibilitySection,
    DataSection,
    DebugSection,
    AboutSection,
  } from './sections';

  import { setSettingsContext, type SettingsContext } from './context';
  import { useTauriSettings } from '$lib/composables/useTauriSettings';
  import { t } from '$lib/locales';
  import { isDev } from '$lib/config/environment';

  interface Props {
    /** Whether the modal is open */
    isOpen?: boolean;
    /** Initial section to scroll to */
    initialSection?: string | null;
    /** Initial search query to pre-fill (eg "input") */
    initialSearch?: string | null;
    /** Callback when modal closes */
    onClose?: () => void;
  }

  let {
    isOpen = $bindable(false),
    initialSection = null,
    initialSearch = null,
    onClose,
  }: Props = $props();

  // Initialize Tauri settings
  const tauriSettings = useTauriSettings();
  const { settings, isLoading, updateSetting: tauriUpdateSetting } = tauriSettings;

  // Picker dialog is mounted once at the page level (+page.svelte) and
  // driven by the `modelDownloadEvents` singleton so it survives modal
  // close + restore via the mini progress indicator. We just open it.

  // Search state. `searchQuery` is bound to the input for instant feedback;
  // `debouncedQuery` lags by SEARCH_DEBOUNCE_MS so the 13-section matcher
  // only re-runs once typing pauses, not on every keystroke.
  let searchQuery = $state('');
  let debouncedQuery = $state('');
  const SEARCH_DEBOUNCE_MS = 100;
  let searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    const next = searchQuery;
    if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    // Empty query commits immediately so the "show all" state isn't
    // delayed when the user clears the field.
    if (next === '') {
      debouncedQuery = '';
      return;
    }
    searchDebounceTimer = setTimeout(() => {
      debouncedQuery = next;
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      if (searchDebounceTimer) clearTimeout(searchDebounceTimer);
    };
  });

  // Reference to header component - used to focus the search input on open
  let settingsHeader: SettingsHeader | null = null;

  // Section search terms mapping
  const sectionSearchTerms: Record<string, string[]> = {
    language: ['language', 'locale', 'translation', 'i18n'],
    model: ['model', 'whisper', 'download', 'ai', 'transcription', 'distil'],
    customModel: ['custom model', 'byo', 'onnx', 'import', 'huggingface'],
    gpu: ['gpu', 'directml', 'acceleration', 'nvidia', 'graphics', 'vram'],
    optimization: ['optimization', 'memory', 'ram', 'unload', 'idle', 'model', 'gaming', 'vram'],
    transform: ['transform', 'grammar', 'ai', 'rewrite', 'rules', 'post-processing'],
    output: ['output', 'clipboard', 'paste', 'copy', 'file', 'save', 'post-record'],
    audio: ['audio', 'microphone', 'mic', 'input', 'device', 'recording device'],
    recording: ['recording', 'vad', 'voice activity', 'silence', 'filler words', 'numbers'],
    hotkeys: ['hotkeys', 'keyboard', 'shortcut', 'keybinding', 'press to talk'],
    customize: [
      'customize',
      'theme',
      'appearance',
      'ui',
      'preview',
      'speed',
      'toast',
      'history',
      'transcript',
      'privacy',
      'copy last',
      'hover',
      'tray',
    ],
    mcpServer: ['mcp', 'cli', 'claude', 'agent', 'transcribe'],
    accessibility: ['accessibility', 'zoom', 'scale', 'scaling', 'text size', 'a11y'],
    data: ['data', 'privacy', 'delete', 'cache', 'storage', 'reset', 'clear'],
    debug: ['debug', 'logs', 'developer', 'diagnostics', 'verbose'],
    about: ['about', 'version', 'license', 'credits', 'update', 'support', 'sponsor', 'donate'],
  };

  // Filter sections against the debounced query so each keystroke doesn't
  // fan out across all 13 matchers + locale lookups.
  function matchesSearch(sectionKey: string, sectionTitle: string): boolean {
    const q = debouncedQuery.trim();
    if (!q) return true;
    const query = q.toLowerCase();
    if (sectionTitle.toLowerCase().includes(query)) return true;
    const terms = sectionSearchTerms[sectionKey] || [];
    return terms.some(term => term.includes(query));
  }

  // Reactive section visibility (uses localized titles for search)
  const sectionVisibility = $derived({
    language: matchesSearch('language', t('languageSettings.title')),
    model: matchesSearch('model', t('modelSettings.title')),
    customModel: matchesSearch('customModel', t('customModel.title')),
    gpu: matchesSearch('gpu', t('gpu.title')),
    optimization: matchesSearch('optimization', t('optimization.title')),
    transform: matchesSearch('transform', t('settings.transform')),
    output: matchesSearch('output', t('settings.output')),
    audio: matchesSearch('audio', t('audioInput.title')),
    recording: matchesSearch('recording', t('settings.recording')),
    hotkeys: matchesSearch('hotkeys', t('hotkeys.title')),
    customize: matchesSearch('customize', t('customize.title')),
    mcpServer: matchesSearch('mcpServer', t('mcpServer.title')),
    accessibility: matchesSearch('accessibility', t('accessibility.title')),
    data: matchesSearch('data', t('data.title')),
    debug: matchesSearch('debug', t('settings.debugTools')),
    about: matchesSearch('about', t('about.title')),
  });

  const hasResults = $derived(Object.values(sectionVisibility).some(Boolean));

  // Clear search when modal closes (sync both, debounce timer cancelled
  // on next effect tick).
  $effect(() => {
    if (!isOpen) {
      searchQuery = '';
      debouncedQuery = '';
    }
  });

  // Pre-fill the search box when opened via a search deep-link
  $effect(() => {
    if (isOpen && initialSearch) {
      searchQuery = initialSearch;
    }
  });

  // Auto-focus the search input when the modal opens (but not when jumping
  // to a section). Wait for tick() so the SettingsHeader is mounted before
  // we call focus(); the previous setTimeout(..., 0) hack would race the
  // 150ms transition and occasionally focus a partially-mounted element.
  $effect(() => {
    if (isOpen && !initialSection) {
      tick().then(() => settingsHeader?.focus());
    }
  });

  // Set up settings context for all child components
  const contextValue: SettingsContext = {
    settings,
    updateSetting: tauriUpdateSetting,
    isLoading,
  };
  setSettingsContext(contextValue);

  // Load settings on mount
  onMount(() => {
    tauriSettings.loadSettings();
    // Re-check disk for both model categories so Settings never shows
    // stale "selected" state after a delete that bypassed the picker
    // (eg user wiped the HF cache directly, or another window deleted).
    modelStore.refreshDownloadedModels();
    transformModelStore.initialize();
  });

  // Scroll to section when panel opens with initialSection
  $effect(() => {
    if (isOpen && initialSection) {
      setTimeout(() => {
        const section = document.getElementById(initialSection);
        if (section) {
          section.scrollIntoView({ behavior: 'smooth', block: 'start' });
          section.classList.add('section-highlight');
          setTimeout(() => section.classList.remove('section-highlight'), 1500);
        }
      }, 100);
    }
  });

  function handleClose() {
    // Validate: if no post-record actions are enabled, enable copy_to_clipboard.
    // Defer the IPC to a microtask so the close path returns immediately -
    // otherwise the bridge round-trip lands in the same frame as isOpen=false
    // and the unmount of all sections, making the dismiss feel laggy.
    const currentSettings = $settings;
    if (currentSettings) {
      const outputTo = Array.isArray(currentSettings.output_to)
        ? currentSettings.output_to
        : currentSettings.output_to
          ? [currentSettings.output_to]
          : [];

      if (outputTo.length === 0) {
        queueMicrotask(() => {
          devLog('[Settings] No post-record actions enabled, enabling copy_to_clipboard');
          tauriUpdateSetting('output_to', ['copy_to_clipboard']);
        });
      }
    }

    isOpen = false;
    onClose?.();
  }

  async function handleReset() {
    await tauriSettings.resetToDefaults();
    await tauriSettings.loadSettings();
  }

  function openModelDownloader() {
    modelDownloadEvents.openModal('transcription');
  }

  function openTransformModelDownloader() {
    modelDownloadEvents.openModal('transformation');
  }
</script>

<ModalWindow {isOpen} onClose={handleClose} hideCloseButton maxWidth="max-w-settings">
  <div class="settings-container">
    <SettingsHeader
      bind:this={settingsHeader}
      title={t('settings.title')}
      onClose={handleClose}
      bind:searchQuery
    />

    <div class="settings-scroll">
      {#if $isLoading}
        <!-- Skeleton loading placeholders -->
        <div class="skeleton-section">
          <Skeleton width="4rem" height="0.625rem" />
          <div class="skeleton-rows">
            {#each Array(3) as _, i (i)}
              <div class="skeleton-row">
                <Skeleton width="40%" height="0.875rem" />
                <Skeleton width="2.5rem" height="1.25rem" rounded="0.625rem" />
              </div>
            {/each}
          </div>
        </div>
        <div class="skeleton-section">
          <Skeleton width="5rem" height="0.625rem" />
          <div class="skeleton-rows">
            {#each Array(4) as _, i (i)}
              <div class="skeleton-row">
                <Skeleton width="55%" height="0.875rem" />
                <Skeleton width="2.5rem" height="1.25rem" rounded="0.625rem" />
              </div>
            {/each}
          </div>
        </div>
        <div class="skeleton-section">
          <Skeleton width="6rem" height="0.625rem" />
          <div class="skeleton-rows">
            {#each Array(2) as _, i (i)}
              <div class="skeleton-row">
                <Skeleton width="45%" height="0.875rem" />
                <Skeleton width="2.5rem" height="1.25rem" rounded="0.625rem" />
              </div>
            {/each}
          </div>
        </div>
      {:else}
        <!--
				Sections are always mounted (not gated by {#if}) so that vite-plugin-svelte
				compiles every section's CSS sidecar at modal open. The {#if}-on-mount pattern
				triggered a cold-cache race where the virtual CSS module request raced ahead
				of the compiled style, causing PostCSS to receive the raw .svelte source and
				choke on `import { onMount }`. See svelte.config.js for prior context.
			-->
        <!--
				Task-flow order: capture (mic -> hotkeys -> recording) -> transcribe
				(model -> custom model -> transform -> gpu) -> output -> preferences ->
				housekeeping.
			-->
        <div hidden={!sectionVisibility.language}><LanguageSection /></div>
        <div hidden={!sectionVisibility.audio}><AudioInputSection /></div>
        <div hidden={!sectionVisibility.hotkeys}><HotkeysSection /></div>
        <div hidden={!sectionVisibility.recording}><RecordingSection /></div>
        <div hidden={!sectionVisibility.model}>
          <ModelSection onChangeModel={openModelDownloader} />
        </div>
        <div hidden={!sectionVisibility.customModel}><CustomModelSection /></div>
        <div hidden={!sectionVisibility.transform}>
          <TransformModelSection onChangeModel={openTransformModelDownloader} />
        </div>
        <div hidden={!sectionVisibility.gpu}><GpuAccelerationSection /></div>
        <div hidden={!sectionVisibility.optimization}><OptimizationSection /></div>
        <div hidden={!sectionVisibility.output}><OutputSection /></div>
        <div hidden={!sectionVisibility.customize}><CustomizeSection /></div>
        <div hidden={!sectionVisibility.accessibility}><AccessibilitySection /></div>
        <div hidden={!sectionVisibility.mcpServer}><McpServerSection /></div>
        <div hidden={!sectionVisibility.data}><DataSection /></div>
        {#if isDev}
          <div hidden={!sectionVisibility.debug}><DebugSection /></div>
        {/if}
        <div hidden={!sectionVisibility.about}><AboutSection /></div>

        {#if searchQuery && !hasResults}
          <div class="no-results">
            <p>{t('settings.noResults', { query: searchQuery })}</p>
          </div>
        {/if}
      {/if}
    </div>

    <SettingsFooter onReset={handleReset} onDone={handleClose} />
  </div>
</ModalWindow>

<!-- Picker dialog is owned by +page.svelte (singleton). Nothing mounted here. -->

<style>
  .settings-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    color: #e4e4e7;
    cursor: default;
  }

  .skeleton-section {
    padding: 1rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .skeleton-rows {
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
  }

  .skeleton-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .no-results {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 3rem 1rem;
    color: #52525b;
    font-size: 0.8125rem;
  }

  /* Scrollable Content - single scroll container */
  .settings-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: scroll;
    overflow-x: clip;
    -webkit-overflow-scrolling: touch;
    padding-right: 0.75rem; /* Buffer for scrollbar */
  }
</style>
