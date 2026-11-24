<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { applyConsoleGate } from '$lib/utils/devLog';

  // Release builds: silence console.log/debug unless the user opted into verbose
  // logging (Settings > About). Runs before any store/service starts chattering.
  applyConsoleGate();

  // Stores
  import {
    pageStore,
    showSettingsPanel,
    settingsInitialSection,
    showImportCustomModelModal,
  } from '$lib/stores/pageStore';
  import { modelStore } from '$lib/stores/modelStore';
  import { transformModelStore } from '$lib/stores/transformModelStore';
  import { transcription } from '$lib/stores/transcription';
  import { executionProviderStore } from '$lib/stores/executionProvider';
  import { t } from '$lib/locales';

  // Composables
  import { useRecording } from '$lib/composables/useRecording';
  import { useAppInit } from '$lib/composables/useAppInit';
  import { useModelDownloadEvents } from '$lib/composables/useModelDownloadEvents';
  import { useRealtimePreview } from '$lib/composables/useRealtimePreview';
  import { useFadeOnIdle } from '$lib/composables/useFadeOnIdle';
  import { useTauriSettings, tauriSettings } from '$lib/composables/useTauriSettings';
  import { initUiScale } from '$lib/composables/useUiScale';
  import { rightProjection } from '$lib/stores/rightProjection';
  import {
    startPageLoadAnimation,
    buttonReady,
    sonarActive,
    brandingReady,
    subtitleReady,
    topRightReady,
  } from '$lib/composables/usePageLoadAnimation';
  import { setupGlobalKeyboard } from '$lib/services/globalKeyboardService';
  import { isUnsupportedMac } from '$lib/stores/platformCache';

  // Onboarding & hints
  import { onboardingStore } from '$lib/stores/onboarding';
  import Onboarding from '$lib/components/Onboarding.svelte';
  import EmptyStateHint from '$lib/components/EmptyStateHint.svelte';

  // Components
  import BrandingHeader from '$lib/components/BrandingHeader.svelte';
  import RecordButton from '$lib/components/RecordButton.svelte';
  import { demoRecording, demoRecordingTime, demoActive, demoCountdown } from '$lib/stores/demo';
  import { resumeDemoIfPending } from '$lib/services/demoService';
  import DemoModal from '$lib/components/debug/DemoModal.svelte';
  import SettingsButton from '$lib/components/SettingsButton.svelte';
  // import ModelAutoDetectToast from '$lib/components/ModelAutoDetectToast.svelte'; // Disabled - awkward UX
  import UpdateBanner from '$lib/components/UpdateBanner.svelte';
  import { isUpdateAvailable } from '$lib/stores/updater';
  import BottomLeftActions from '$lib/components/BottomLeftActions.svelte';
  import ModelSelectorButton from '$lib/components/ModelSelectorButton.svelte';
  import MiniDownloadProgress from '$lib/components/MiniDownloadProgress.svelte';
  import TranscriptionPreviewArea from '$lib/components/TranscriptionPreviewArea.svelte';
  import VersionIndicator from '$lib/components/VersionIndicator.svelte';
  import DebugPanel from '$lib/components/debug/DebugPanel.svelte';
  import ModelDownloadProgress from '$lib/components/ModelDownloadProgress.svelte';
  import { SettingsModal } from '$lib/components/settings';
  import ImportCustomModelModal from '$lib/components/ImportCustomModelModal.svelte';
  import AccessibilityPermissionModal from '$lib/components/AccessibilityPermissionModal.svelte';
  import UpdateModal from '$lib/components/UpdateModal.svelte';
  import LoadingOverlay from '$lib/components/LoadingOverlay.svelte';
  import {
    showUpdateModal,
    updateVersion,
    updateReleaseNotes,
    resolveUpdatePrompt,
    downloadAndInstall,
    updateStatus,
  } from '$lib/stores/updater';
  import UnsupportedPlatformBanner from '$lib/components/UnsupportedPlatformBanner.svelte';
  import OutputToastsContainer from '$lib/components/OutputToastsContainer.svelte';
  import ModelAutoSwitchToast from '$lib/components/ModelAutoSwitchToast.svelte';
  // Initialize composables
  const { loadSettings: loadTauriSettings } = useTauriSettings();
  let uiScaleCleanup: (() => void) | null = null;
  const appInit = useAppInit();
  const downloadEvents = useModelDownloadEvents();
  const realtimePreview = useRealtimePreview();
  const modelIndicatorFade = useFadeOnIdle({ delay: 5000 });

  // Extract stores from composables for $ prefix usage
  const appInitIsInitializing = appInit.isInitializing;
  const appInitAppVersion = appInit.appVersion;
  const appInitGpuInfo = appInit.gpuInfo;
  const appInitOsInfo = appInit.osInfo;
  const downloadIsModalOpen = downloadEvents.isModalOpen;
  const downloadIsActive = downloadEvents.isDownloadActive;
  const downloadCurrentCategory = downloadEvents.currentCategory;
  const downloadProgress = downloadEvents.progress;
  const downloadSpeed = downloadEvents.speed;
  const downloadTimeRemaining = downloadEvents.timeRemaining;
  const downloadStatus = downloadEvents.status;

  // Recording composable
  const {
    isRecording: isRecordingStore,
    isProcessing: isProcessingStore,
    recordingTime: recordingTimeStore,
    errorMessage: errorMessageStore,
    serverStatus: serverStatusStore,
    isOpenMicActive: isOpenMicActiveStore,
    isOpenMicListening: isOpenMicListeningStore,
    isOpenMicSilenceCounting: isOpenMicSilenceCountingStore,
    toggleRecording,
    isStartPending,
    getStartSeq,
    cancelRecording,
    formatTime,
    cleanup: cleanupRecording,
  } = useRecording(
    () => $modelStore.modelExists,
    () => downloadEvents.openModal()
  );

  // Subscribe to recording state
  // Demo (DebugPanel > Demo) fakes a recording without touching the real recorder.
  $: isRecording = $isRecordingStore || $demoRecording;
  $: isProcessing = $isProcessingStore;
  $: recordingTime = $demoRecording ? $demoRecordingTime : $recordingTimeStore;
  $: errorMessage = $errorMessageStore;
  $: serverStatus = $serverStatusStore;
  $: isOpenMicActive = $isOpenMicActiveStore;
  $: isOpenMicListening = $isOpenMicListeningStore;
  $: isOpenMicSilenceCounting = $isOpenMicSilenceCountingStore;

  // Reactive settings
  $: showRealtimePreview = $tauriSettings?.show_realtime_preview !== false;

  // History label state: right side lights up when the left-chip projection is active.
  let isShowingHistoryLeft = false;
  $: isShowingHistoryRight = $rightProjection !== null;
  $: isShowingHistory =
    (!isRecording || isOpenMicActive) &&
    !isProcessing &&
    (isShowingHistoryRight || isShowingHistoryLeft);

  // Unsupported platform detection (Intel Mac) - reactive via Svelte store
  $: isUnsupported = $isUnsupportedMac;

  // Model state
  $: currentSelectedModel = $modelStore.selectedModel;

  // Model selector glow effect
  let modelSelectorGlowing = false;
  function triggerModelSelectorGlow() {
    modelSelectorGlowing = true;
    setTimeout(() => (modelSelectorGlowing = false), 1500);
  }

  // Track model changes to trigger glow
  let previousSelectedModel: string | null = null;
  $: if (
    currentSelectedModel &&
    previousSelectedModel !== null &&
    currentSelectedModel !== previousSelectedModel
  ) {
    triggerModelSelectorGlow();
  }
  // Read by the `$:` block above; legacy reactive statements are invisible to this rule
  // eslint-disable-next-line no-useless-assignment
  $: previousSelectedModel = currentSelectedModel;

  // Start fade timer when model exists
  $: if ($modelStore.modelExists) {
    modelIndicatorFade.startFade();
  }

  // Error action callbacks for RecordButton
  const errorActions = {
    openDownloadModal: () => downloadEvents.openModal(),
    openGpuSettings: () => pageStore.openSettings('gpu-acceleration'),
    openAudioSettings: () => pageStore.openSettings('audio-input'),
    retry: () => handleRecordClick(),
  };

  // Stop recording when the settings modal opens. Opening settings from a
  // hotkey or menu shouldn't silently keep a mic session running in the
  // background -cancel any in-progress recording so the audio buffer is
  // discarded (no accidental transcription into settings-modal text fields).
  let prevShowSettingsPanel = false;
  $: {
    if ($showSettingsPanel && !prevShowSettingsPanel && (isRecording || isProcessing)) {
      console.log('[Page] Settings opened while recording -cancelling');
      cancelRecording();
    }
    // Read by the same `$:` block on its next run; invisible to the rule
    // eslint-disable-next-line no-useless-assignment
    prevShowSettingsPanel = $showSettingsPanel;
  }

  // Recording handler - fully offline, no account needed
  function handleRecordClick() {
    realtimePreview.reset();
    transcription.clearCurrentResult();
    toggleRecording();
  }

  // Cleanup
  let cleanup: (() => void) | null = null;
  let keyboardCleanup: (() => void) | null = null;
  let importModalRef: { close: () => void } | undefined;

  // Disable browser right-click context menu (native desktop app)
  function preventContextMenu(e: MouseEvent) {
    e.preventDefault();
  }

  onMount(async () => {
    document.addEventListener('contextmenu', preventContextMenu);
    startPageLoadAnimation();
    if (import.meta.env.DEV) resumeDemoIfPending();

    uiScaleCleanup = initUiScale();

    cleanup = await appInit.init({
      setupDownloadListeners: async listen => {
        downloadEvents.onDownloadComplete = triggerModelSelectorGlow;
        return downloadEvents.setupListeners(listen);
      },
      onTranscriptionSegment: (text, isFinal, transformInfo) => {
        realtimePreview.handleSegment(text, isFinal, $tauriSettings, transformInfo);
      },
      onHotkeyToggle: handleRecordClick,
      isHotkeyDisabled: () => $isProcessingStore || $serverStatusStore === 'connecting',
      hotkeyMode: {
        mode: () =>
          ($tauriSettings?.recording_mode ?? 'push_to_talk') === 'push_to_talk'
            ? ($tauriSettings?.hotkey_mode ?? 'hybrid')
            : 'toggle',
        isRecording: () => get(isRecordingStore),
        start: { isPending: isStartPending, seq: getStartSeq },
      },
      onDownloadComplete: triggerModelSelectorGlow,
      loadTauriSettings,
    });

    // Setup global keyboard shortcuts
    if (appInit.invoke) {
      keyboardCleanup = setupGlobalKeyboard({
        invoke: appInit.invoke,
        isAnyModalOpen: () => pageStore.isAnyModalOpen() || get(downloadEvents.isModalOpen),
        closeSettings: () => pageStore.closeSettings(),
        closeDownloadModal: () => downloadEvents.closeModal(),
        backOrCloseImportCustomModelModal: () => importModalRef?.close(),
        cancelRecording: () => cancelRecording(),
        modalStates: {
          showSettingsPanel: () => pageStore.getShowSettingsPanel(),
          showImportCustomModelModal: () => get(showImportCustomModelModal),
          isDownloadingModel: () => get(downloadEvents.isModalOpen),
          isRecording: () => get(isRecordingStore) || get(isOpenMicActiveStore),
        },
      });
    }
  });

  onDestroy(() => {
    document.removeEventListener('contextmenu', preventContextMenu);
    if (cleanup) cleanup();
    if (keyboardCleanup) keyboardCleanup();
    cleanupRecording();
    realtimePreview.cleanup();
    modelIndicatorFade.cleanup();
    if (uiScaleCleanup) uiScaleCleanup();
  });
</script>

<svelte:head>
  <title>Echophrase - Privacy-1st gpu speech→text</title>
</svelte:head>

<!-- h-full (not h-screen): body owns the viewport height and compensates the UI-scale transform -->
<main
  class="h-full app-backdrop flex flex-col items-center justify-center p-8 pb-24 relative overflow-hidden"
>
  <!-- The backdrop bloom itself lives in .app-backdrop (app.css), centered on the
       record button. Corner blobs are deliberately absent: they pulled the eye to
       the edges and fought the centered layout. -->
  <div class="absolute inset-0 pointer-events-none">
    <!-- Noise overlay to prevent gradient banding on subtle dark gradients -->
    <div class="absolute inset-0 opacity-[0.015] mix-blend-overlay bg-noise"></div>
  </div>

  <!-- Toasts & Banners -->
  <!-- <ModelAutoDetectToast /> -->
  <!-- Disabled - awkward UX before model download -->
  <ModelAutoSwitchToast />
  <UnsupportedPlatformBanner />
  {#if !isUnsupported}
    {#if $isUpdateAvailable}
      <UpdateBanner />
    {/if}
  {/if}

  <!-- Top-Right: Settings -->
  <div
    class="fixed right-4 z-40 flex items-center gap-2 transition-all duration-300 ease-out-quint {$topRightReady
      ? 'opacity-100'
      : 'opacity-0'} {isUnsupported ? 'top-44' : 'top-4'}"
  >
    <SettingsButton onClick={() => pageStore.toggleSettings()} />
  </div>

  <!-- Center: Branding Header -->
  <div
    class="flex flex-col items-center transition-opacity duration-600 ease-out-cubic {$brandingReady
      ? 'opacity-100'
      : 'opacity-0 pointer-events-none'}"
  >
    <BrandingHeader
      {isRecording}
      {recordingTime}
      {formatTime}
      subtitleReady={$subtitleReady}
      {isOpenMicActive}
      {isShowingHistory}
    />
  </div>

  <!-- Center: Recording Button with Speed Toast -->
  <div
    class="relative flex items-start justify-center transition-opacity duration-300 ease-out-cubic {$buttonReady
      ? 'opacity-100'
      : 'opacity-0 pointer-events-none'}"
  >
    <!-- Output Toasts + left-side history flyout (speed entries live in here now) -->
    <OutputToastsContainer onHistoryHoverChange={h => (isShowingHistoryLeft = h)} />

    <RecordButton
      {isRecording}
      {isProcessing}
      isModelLoading={$modelStore.isPreloading || $executionProviderStore.isLoading}
      toggleRecording={handleRecordClick}
      {errorMessage}
      {formatTime}
      {serverStatus}
      showIntroPulse={$sonarActive}
      {errorActions}
      recordingMode={$tauriSettings?.recording_mode || 'push_to_talk'}
      {isOpenMicListening}
      {isOpenMicSilenceCounting}
      silenceTimeout={$tauriSettings?.silence_timeout ?? 5}
      {recordingTime}
    />

    <!-- Transcription Preview Area (right of button) -->
    {#if showRealtimePreview}
      <TranscriptionPreviewArea
        realtimeText={realtimePreview.text}
        realtimeVisible={realtimePreview.isVisible}
        realtimeFading={realtimePreview.isFading}
        realtimePageIndex={realtimePreview.pageIndex}
        isShowingOriginal={realtimePreview.isShowingOriginal}
        isTransforming={realtimePreview.isTransforming}
        isSilenceResult={realtimePreview.isSilenceResult}
      />
    {/if}
  </div>

  <!-- Empty state hint. In normal flow (fixed-height slot) so the centered group
       is logo + button + hint, and nothing jumps when the hint fades away. -->
  {#if import.meta.env.DEV}
    <DemoModal />
  {/if}

  <!-- Scripted-demo countdown (10..3; 2 and 1 stay hidden so they never reach the recording). -->
  {#if $demoCountdown !== null}
    <div
      class="fixed top-3 left-1/2 -translate-x-1/2 z-50 pointer-events-none text-sm tabular-nums text-zinc-500/70"
      aria-hidden="true"
    >
      {$demoCountdown}
    </div>
  {/if}

  <EmptyStateHint
    isRecording={isRecording || $demoActive}
    {isProcessing}
    onOpenDownloadModal={() => downloadEvents.openModal()}
  />

  {#if !isUnsupported}
    <!-- Bottom-Left: Action Buttons -->
    <BottomLeftActions
      {isRecording}
      {isProcessing}
      {isOpenMicActive}
      onHistoryHoverChange={h => (isShowingHistoryLeft = h)}
    />

    <!-- Bottom-Center: Model Selector -->
    <ModelSelectorButton
      onOpenDownloadModal={() => downloadEvents.openModal()}
      glowing={modelSelectorGlowing}
    />

    <!-- Mini Download Progress (when modal closed). Custom BYO downloads now
       hand off to the SAME shared ModelDownloadProgress dialog registry
       downloads use (see ImportCustomModelModal.svelte's downloadHfFile +
       useModelDownloadEvents.ts's beginCustomDownload), so this needs no
       custom-vs-registry special-casing - $downloadIsModalOpen and
       downloadEvents.openModal() already cover both. -->
    <MiniDownloadProgress
      isDownloadActive={$downloadIsActive}
      isModalOpen={$downloadIsModalOpen}
      progress={$downloadProgress}
      speed={$downloadSpeed}
      timeRemaining={$downloadTimeRemaining}
      onOpenModal={() => downloadEvents.openModal()}
    />
  {/if}

  <!-- Model Preload Progress (post-startup) -->
  {#if !isUnsupported && ($modelStore.isPreloading || $executionProviderStore.isLoading) && !$appInitIsInitializing}
    <div class="fixed bottom-14 left-1/2 -translate-x-1/2 w-44 z-50">
      <div
        class="bg-zinc-800/90 backdrop-blur-sm rounded-lg p-2.5 border border-zinc-700/50 shadow-lg"
      >
        <div class="text-xs text-zinc-400 mb-1.5 flex items-center gap-1.5">
          <svg class="w-3 h-3 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"
            ></circle>
            <path
              class="opacity-75"
              fill="currentColor"
              d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
            ></path>
          </svg>
          {$executionProviderStore.isLoading ? t('gpu.detectingGpu') : t('model.loadingModel')}
        </div>
        <div class="h-1.5 bg-zinc-700 rounded-full overflow-hidden">
          <div
            class="h-full bg-gradient-to-r from-lime-400 to-emerald-500 loading-progress-fill"
          ></div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Debug Panel (dev only) -->
  {#if import.meta.env.DEV}
    <DebugPanel />
  {/if}

  <!-- Modals -->
  <!-- Single page-level download dialog. Category + active store route
       reactively from the singleton composable, so opening from voice
       settings vs transform settings vs minimising/restoring all share
       the same dialog instance. -->
  <ModelDownloadProgress
    isDownloading={$downloadIsModalOpen}
    progress={$downloadProgress}
    downloadSpeed={$downloadSpeed}
    timeRemaining={$downloadTimeRemaining}
    status={$downloadStatus}
    category={$downloadCurrentCategory}
    activeStore={$downloadCurrentCategory === 'transformation' ? transformModelStore : modelStore}
    onClose={() => downloadEvents.closeModal()}
  />

  <SettingsModal
    bind:isOpen={$showSettingsPanel}
    initialSection={$settingsInitialSection}
    onClose={() => pageStore.closeSettings()}
  />

  <!-- Explains WHY macOS asks for Accessibility before the OS's own vague prompt can -->
  <AccessibilityPermissionModal />

  <ImportCustomModelModal
    bind:this={importModalRef}
    bind:isOpen={$showImportCustomModelModal}
    onClose={() => pageStore.closeImportCustomModelModal()}
    onImported={() => pageStore.closeImportCustomModelModal({ skipReturn: true })}
  />

  <UpdateModal
    isOpen={$showUpdateModal}
    version={$updateVersion ?? ''}
    releaseNotes={$updateReleaseNotes ?? ''}
    onUpdate={() => downloadAndInstall()}
    onLater={() => resolveUpdatePrompt(false)}
  />

  <!--
    Install blocker: the instant "Update Now" is clicked, downloadAndInstall()
    flips updateStatus to 'downloading' synchronously (before its first await),
    so this full-screen overlay (z-index 9999, blocks pointer events + ESC) paints
    in the same tick - covering the still-open UpdateModal underneath and the
    record button, closing the ~1s window where the user could start a recording
    mid-update. The modal is intentionally left open in the background; if the
    install errors out, the overlay disappears and the modal is there to retry.
  -->
  <LoadingOverlay
    visible={$updateStatus === 'downloading' || $updateStatus === 'ready'}
    message={t('update.installing')}
    subtext={t('update.installingSub')}
  />

  <!-- First-run Onboarding -->
  {#if $onboardingStore && !$onboardingStore.completed && !$appInitIsInitializing}
    <Onboarding
      onOpenDownloadModal={() => downloadEvents.openModal()}
      isModalOpen={$downloadIsModalOpen}
      isDownloadActive={$downloadIsActive}
    />
  {/if}

  <!-- Version Indicator -->
  <VersionIndicator
    appVersion={$appInitAppVersion}
    gpuInfo={$appInitGpuInfo}
    osInfo={$appInitOsInfo}
    {currentSelectedModel}
  />
</main>
