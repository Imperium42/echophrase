<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Audio Input Section
   *
   * Audio input settings.
   * Contains: Input Device selector, plus the Open Mic tuning block (Silence
   * Timeout + Open Mic Threshold with live VAD preview) shown only in open-mic
   * mode - neither reaches the push-to-talk path, which runs no VAD.
   */

  import { onMount, onDestroy } from 'svelte';
  import { SettingsSection, HintTooltip, SettingSlider, FeatureBadge } from '../primitives';
  import { getSettingsContext } from '../context';
  import { mustInvoke, safeInvoke } from '$lib/ipc/invoke';
  import { useVadTest } from '$lib/composables/useVadTest';
  import VadMeter from './audio/VadMeter.svelte';
  import { t, translate } from '$lib/locales';
  import { isLinux } from '$lib/utils/platform';
  import { Icon } from '$lib/icons';
  import { platformCache } from '$lib/stores/platformCache';

  const { settings, updateSetting } = getSettingsContext();
  // Recording mode toggle is always shown so users can switch between
  // push-to-talk and open-mic on any platform. The recommended badge and
  // tooltip text vary by platform - on Linux open-mic is recommended due
  // to spotty global hotkey support across distros.
  const isLinuxPlatform = isLinux();
  // WSL passthrough: the input device is always the WSLg RDP passthrough
  // source, and the real Windows mic it carries is latched by msrdc.exe at
  // WSL session start. Changing the Windows default mid-session does nothing
  // until `wsl --shutdown` + reopen. Surface that via the same penguin badge
  // used next to Open Mic, placed next to the device picker instead.
  const isWsl = platformCache.isWsl;

  // Audio input devices
  interface AudioInputDevice {
    deviceId: string;
    label: string;
  }
  let audioInputDevices = $state<AudioInputDevice[]>([]);
  let selectedInputDevice = $state('default');
  let isLoadingDevices = $state(false);
  let deviceError = $state('');

  // Recording mode state
  let recordingMode = $state<'push_to_talk' | 'open_mic'>('push_to_talk');
  let silenceTimeout = $state(5);

  // Sync recording mode from settings
  $effect(() => {
    if ($settings?.recording_mode != null) {
      recordingMode = $settings.recording_mode;
    }
  });

  // Sync silence timeout from settings
  $effect(() => {
    if ($settings?.silence_timeout != null) {
      silenceTimeout = $settings.silence_timeout;
    }
  });

  function setRecordingMode(mode: 'push_to_talk' | 'open_mic') {
    recordingMode = mode;
    updateSetting('recording_mode', mode);
  }

  function setSilenceTimeout(value: number) {
    silenceTimeout = Math.max(2, Math.min(10, value));
    updateSetting('silence_timeout', silenceTimeout);
  }

  // Voice threshold state (0-100)
  let thresholdValue = $state(50);

  // A HIGHER threshold means the VAD demands more confidence before it counts
  // audio as speech, so a high value makes open mic miss quiet or soft-spoken
  // starts entirely. Ramp green -> yellow -> red toward that risky end.
  const THRESHOLD_CAUTION_ABOVE = 80;
  const THRESHOLD_RAMP = [
    { at: 0, color: '#84cc16' },
    { at: 60, color: '#84cc16' },
    { at: 80, color: '#eab308' },
    { at: 100, color: '#ef4444' },
  ];
  const isThresholdHigh = $derived(thresholdValue > THRESHOLD_CAUTION_ABOVE);

  // VAD live-test (state + Tauri lifecycle owned by composable)
  const vadTest = useVadTest();
  const isTestRunning = vadTest.isRunning;
  const vadLevel = vadTest.level;
  const peakLevel = vadTest.peak;
  const peakOpacity = vadTest.peakOpacity;
  const isSpeechDetected = vadTest.isSpeech;

  // Switching to Push to Talk unmounts the threshold block, including the Test
  // button. Stop any running test first so it doesn't keep capturing with no
  // visible way to turn it off.
  $effect(() => {
    if (recordingMode !== 'open_mic' && $isTestRunning) {
      void vadTest.stop();
    }
  });

  // Sync threshold from settings
  $effect(() => {
    if ($settings?.voice_threshold != null) {
      thresholdValue = $settings.voice_threshold;
    }
  });

  // Sync input device from settings
  $effect(() => {
    if ($settings?.input_device != null) {
      selectedInputDevice = $settings.input_device;
    }
  });

  onMount(async () => {
    await loadAudioDevices();
  });

  onDestroy(() => {
    // Composable owns the IPC + listener teardown; cleanup() drops timers
    // and detaches the event listener without firing IPC (faster on close).
    vadTest.cleanup();
  });

  async function loadAudioDevices() {
    isLoadingDevices = true;
    deviceError = '';
    try {
      // Try Tauri command first for native device enumeration
      try {
        const nativeDevices =
          await mustInvoke<{ id: string; name: string }[]>('get_audio_input_devices');
        if (nativeDevices && nativeDevices.length > 0) {
          audioInputDevices = [
            { deviceId: 'default', label: translate('audioInput.systemDefault') },
            ...nativeDevices.map(d => ({ deviceId: d.id, label: d.name })),
          ];
          return;
        }
      } catch {
        // Native enumeration not available, fall through to browser API
      }

      // Fallback to browser API (without requesting permission)
      if (navigator.mediaDevices) {
        const devices = await navigator.mediaDevices.enumerateDevices();
        const inputDevices = devices.filter(device => device.kind === 'audioinput');

        // Generate labels: use real label if available, otherwise numbered placeholder
        let micNumber = 1;
        const mappedDevices = inputDevices.map(device => ({
          deviceId: device.deviceId,
          label:
            device.label ||
            translate('audioInput.microphoneFallback', { number: String(micNumber++) }),
        }));

        audioInputDevices = [
          { deviceId: 'default', label: translate('audioInput.systemDefault') },
          ...mappedDevices.filter(d => d.deviceId !== 'default'),
        ];
      } else {
        throw new Error('No audio device API available');
      }
    } catch (err) {
      console.error('[Audio Devices] Failed to enumerate:', err);
      deviceError = translate('audioInput.deviceDetectError');
      audioInputDevices = [{ deviceId: 'default', label: translate('audioInput.systemDefault') }];
    } finally {
      isLoadingDevices = false;
    }
  }

  // Full label for the currently selected device, used as a title attr so
  // long names (e.g. "MOTIV Mix Virtual Output (Shure Virtual Audio)") are
  // still readable via native tooltip if the select truncates them.
  const selectedDeviceLabel = $derived(
    audioInputDevices.find(d => d.deviceId === selectedInputDevice)?.label ?? ''
  );

  function handleDeviceChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    selectedInputDevice = target.value;
    updateSetting('input_device', target.value);
  }

  // Throttle `set_vad_threshold` IPC during slider drag: at 60Hz mousemove,
  // the previous unthrottled call queued hundreds of Rust invocations per
  // second. ~50ms is well under perceptual latency for the live preview
  // while keeping IPC pressure sane.
  let vadThrottleTimer: ReturnType<typeof setTimeout> | null = null;
  let vadThrottleQueued: number | null = null;

  function flushVadThreshold() {
    vadThrottleTimer = null;
    const v = vadThrottleQueued;
    vadThrottleQueued = null;
    if (v !== null) {
      safeInvoke<void>('set_vad_threshold', { threshold: v });
    }
  }

  function pushVadThreshold(value: number) {
    vadThrottleQueued = value;
    if (vadThrottleTimer === null) {
      vadThrottleTimer = setTimeout(flushVadThreshold, 50);
    }
  }

  function setThreshold(value: number, save = true) {
    thresholdValue = Math.max(0, Math.min(100, value));
    if (save) {
      updateSetting('voice_threshold', thresholdValue);
    }
    // Update VAD threshold in real-time if test is running
    if (isTestRunning) {
      pushVadThreshold(thresholdValue);
    }
  }

  // Final commit on pointerup - persist, flush any queued throttle, and
  // send the canonical value.
  function commitThreshold(value: number) {
    setThreshold(value, true);
    if (isTestRunning) {
      if (vadThrottleTimer !== null) {
        clearTimeout(vadThrottleTimer);
        vadThrottleTimer = null;
      }
      vadThrottleQueued = null;
      safeInvoke<void>('set_vad_threshold', { threshold: thresholdValue });
    }
  }

  const toggleVadTest = () => vadTest.toggle(selectedInputDevice);
</script>

<SettingsSection id="audio-input" title={t('audioInput.title')} icon="audio-input">
  <!-- Audio Input Device Selector -->
  <div class="setting-row">
    <span class="setting-label">
      {t('audioInput.inputDevice')}
      {#if isWsl}
        <span
          class="linux-penguin"
          title={translate('audioInput.wslPassthroughTooltip')}
          aria-label={translate('audioInput.wslPassthroughTooltip')}
        >
          <Icon name="brand-linux" size={14} />
        </span>
      {/if}
    </span>
    <div class="device-select-row">
      {#if isLoadingDevices}
        <div class="device-spinner" aria-label={t('audioInput.detectingDevices')}></div>
      {/if}
      <div class="select-wrapper select-wrapper-md" title={selectedDeviceLabel}>
        <select
          class="select-field"
          value={selectedInputDevice}
          onchange={handleDeviceChange}
          disabled={isLoadingDevices}
        >
          {#if isLoadingDevices}
            <option value="default">{t('audioInput.loadingDevices')}</option>
          {:else if audioInputDevices.length === 0}
            <option value="default">{t('audioInput.systemDefault')}</option>
          {:else}
            {#each audioInputDevices as device}
              <option value={device.deviceId} title={device.label}>
                {device.label}
              </option>
            {/each}
          {/if}
        </select>
        <svg class="select-arrow" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"
          ></path>
        </svg>
      </div>
    </div>
  </div>
  {#if deviceError}
    <div class="device-error">{deviceError}</div>
  {/if}

  <!-- Recording Mode Toggle -->
  <div class="setting-row">
    <span class="setting-label">
      {t('audioInput.recordingMode')}
      <HintTooltip
        hint={isLinuxPlatform
          ? translate('audioInput.recordingModeHintLinux')
          : translate('audioInput.recordingModeHint')}
      />
    </span>
    <div class="mode-toggle">
      <button
        class="mode-btn"
        class:active={recordingMode === 'push_to_talk'}
        onclick={() => setRecordingMode('push_to_talk')}
        type="button"
      >
        {t('audioInput.pushToTalk')}
      </button>
      <button
        class="mode-btn open-mic-btn"
        class:active={recordingMode === 'open_mic'}
        onclick={() => setRecordingMode('open_mic')}
        type="button"
      >
        {#if isLinuxPlatform}
          <span
            class="linux-penguin"
            title={translate('audioInput.openMicTooltip')}
            aria-label={translate('audioInput.openMicTooltip')}
          >
            <Icon name="brand-linux" size={14} />
          </span>
        {/if}
        {t('audioInput.openMic')}
        <FeatureBadge variant="beta" />
      </button>
    </div>
  </div>

  <!-- Open mic tuning. Both of these feed `start_recording_with_vad` only -
       push-to-talk's `start_recording` runs no VAD - so they are hidden
       entirely in Push to Talk rather than shown as no-ops. -->
  {#if recordingMode === 'open_mic'}
    <!-- Silence Timeout -->
    <div class="setting-row">
      <span class="setting-label">{t('audioInput.silenceTimeout')}</span>
      <span class="setting-value">{silenceTimeout}s</span>
    </div>
    <div class="threshold-hint">{t('audioInput.silenceTimeoutHint')}</div>
    <SettingSlider
      value={silenceTimeout}
      min={2}
      max={10}
      step={1}
      ariaLabel={t('audioInput.silenceTimeoutAria')}
      labels={['2s', '6s', '10s']}
      oninput={v => setSilenceTimeout(v)}
      oncommit={v => setSilenceTimeout(v)}
    />

    <!-- Open Mic Threshold -->
    <div class="setting-row">
      <span class="setting-label">{t('audioInput.voiceThreshold')}</span>
      <div class="threshold-controls">
        <span class="setting-value" class:caution={isThresholdHigh}>{thresholdValue}%</span>
        <button
          class="test-button"
          class:testing={$isTestRunning}
          onclick={toggleVadTest}
          type="button"
        >
          {$isTestRunning ? t('audioInput.stop') : t('audioInput.test')}
        </button>
      </div>
    </div>
    <div class="threshold-hint">{t('audioInput.thresholdHint')}</div>

    <!-- Live Audio Meter (visible during test) -->
    {#if $isTestRunning}
      <VadMeter
        level={$vadLevel}
        peak={$peakLevel}
        peakOpacity={$peakOpacity}
        isSpeech={$isSpeechDetected}
        {thresholdValue}
      />
    {/if}

    <SettingSlider
      value={thresholdValue}
      variant="ramp"
      rampStops={THRESHOLD_RAMP}
      ariaLabel={t('audioInput.voiceThresholdAria')}
      labels={['0%', '50%', '100%']}
      oninput={v => setThreshold(v, false)}
      oncommit={commitThreshold}
    />
    {#if isThresholdHigh}
      <p class="threshold-caution">{t('audioInput.thresholdHighCaution')}</p>
    {/if}
  {/if}
</SettingsSection>

<style>
  .device-select-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1;
    min-width: 0;
  }

  .device-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(132, 204, 22, 0.2);
    border-top-color: #84cc16;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .device-error {
    font-size: 0.6875rem;
    color: #f87171;
    margin-top: -0.25rem;
    margin-bottom: 0.5rem;
  }

  /* `.setting-row` + `.setting-label` come from app.css (shared) */

  .setting-value {
    font-size: 0.8125rem;
    font-weight: 500;
    color: #84cc16;
    transition: color 0.15s ease;
  }

  /* Matches the red end of the slider ramp. */
  .setting-value.caution {
    color: #ef4444;
  }

  .threshold-controls {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .test-button {
    padding: 0.25rem 0.625rem;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    font-weight: 500;
    color: #a1a1aa;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease,
      border-color 0.15s ease;
  }

  .test-button:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #d4d4d8;
  }

  .test-button.testing {
    background: rgba(132, 204, 22, 0.15);
    border-color: rgba(132, 204, 22, 0.3);
    color: #84cc16;
  }

  .select-wrapper {
    position: relative;
    min-width: 0;
  }

  /* Widened to fill the row instead of leaving a large gap next to the
	   "Input Device" label - real device names (e.g. "MOTIV Mix Virtual
	   Output (Shure Virtual Audio)") were being truncated in the old 180px
	   cap. Grows with the row up to the settings panel width; the `title`
	   attr on the wrapper + each <option> covers any remaining truncation. */
  .select-wrapper-md {
    flex: 1;
    min-width: 140px;
    max-width: 100%;
  }

  .select-field {
    width: 100%;
    padding: 0.375rem 1.75rem 0.375rem 0.625rem;
    background: rgba(255, 255, 255, 0.04);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    color: #a1a1aa;
    cursor: pointer;
    appearance: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .select-field:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
    color: #d4d4d8;
  }

  .select-field:focus {
    outline: none;
    background: rgba(255, 255, 255, 0.08);
  }

  .select-field:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .select-arrow {
    position: absolute;
    right: 0.5rem;
    top: 50%;
    transform: translateY(-50%);
    width: 0.875rem;
    height: 0.875rem;
    color: #52525b;
    pointer-events: none;
  }

  /* Threshold Hint */
  .threshold-hint {
    font-size: 0.6875rem;
    color: #52525b;
    margin-top: -0.5rem;
    margin-bottom: 0.5rem;
  }

  /* Brighter than `.threshold-hint` - this one is a warning, not a description. */
  .threshold-caution {
    margin: 0 0 0.5rem;
    font-size: 0.6875rem;
    line-height: 1.4;
    color: #a1a1aa;
  }

  /* Audio meter visualization lives in <VadMeter/> */

  /* Sliders live in the shared <SettingSlider/> primitive */

  /* Recording Mode Toggle */
  .mode-toggle {
    display: flex;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 0.375rem;
    padding: 2px;
    gap: 2px;
  }

  .mode-btn {
    padding: 0.25rem 0.5rem;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    font-weight: 500;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
    display: flex;
    align-items: center;
    gap: 0.375rem;
    white-space: nowrap;
  }

  .mode-btn:hover {
    color: #a1a1aa;
  }

  .mode-btn.active {
    background: rgba(255, 255, 255, 0.08);
    color: #e4e4e7;
  }

  /* Linux-only hint penguin sits inline to the left of "Open Mic". The
	   help cursor + native title tooltip surfaces the rationale without
	   stealing layout space the way the old floating badge did. */
  .linux-penguin {
    display: inline-flex;
    align-items: center;
    margin-left: 0.4em;
    margin-right: 0.4em;
    line-height: 1;
    cursor: help;
    transition: transform 120ms ease;
  }

  .linux-penguin:hover {
    transform: scale(1.15);
  }
</style>
