<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Recording Section
   *
   * Max duration slider (up to 3 minutes) plus recording
   * enhancement toggles (filler words, numeric digits). Audio input lives in
   * AudioInputSection.
   */

  import { SettingsSection, SettingToggle, SettingSlider } from '../primitives';
  import { getSettingsContext } from '../context';
  import { t } from '$lib/locales';

  const { settings, updateSetting } = getSettingsContext();

  let durationValue = $state(30);

  // Non-linear: 5s granularity through the common 1-minute range, then coarse
  // jumps out to 3 minutes. The slider runs over ARRAY INDICES, not seconds, so
  // every step is one equal notch - a linear 5-180s scale would squeeze the
  // range people actually use into the first third of the bar.
  //
  // The 3 minute ceiling is a memory limit, not a tier limit: the model's working
  // memory grows much faster than clip length, and a single 5 minute clip pushed
  // a real install to ~4.5 GB (see BENCHMARKS.md). Keep this in step with
  // `MAX_RECORDING_TIME_SECONDS` (useRecording.ts) and the Rust feature-gate limits.
  const DURATION_STEPS = [5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 90, 120, 180];
  const LAST_INDEX = DURATION_STEPS.length - 1;

  // Past 1 minute the wait gets real: transcription is not streaming, so
  // nothing appears until the whole clip finishes (and a CPU-only machine runs
  // near realtime). Ramp green -> yellow -> red so the cost is visible before
  // the user commits to it: yellow once past 1m, solid red from 2m to 3m.
  const CAUTION_AFTER_SECONDS = 60;
  const DANGER_FROM_SECONDS = 120;
  const pct = (seconds: number) => (DURATION_STEPS.indexOf(seconds) / LAST_INDEX) * 100;

  const DURATION_RAMP = [
    { at: 0, color: '#84cc16' },
    { at: pct(60), color: '#84cc16' },
    { at: pct(90), color: '#eab308' },
    { at: pct(120), color: '#ef4444' },
    { at: 100, color: '#ef4444' },
  ];

  const durationIndex = $derived(Math.max(0, DURATION_STEPS.indexOf(durationValue)));
  const isLongDuration = $derived(durationValue > CAUTION_AFTER_SECONDS);
  const isDangerDuration = $derived(durationValue >= DANGER_FROM_SECONDS);

  function formatDuration(seconds: number): string {
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    const rest = seconds % 60;
    return rest === 0 ? `${minutes}m` : `${minutes}m${rest}s`;
  }

  // Sync duration from settings. Snap to the nearest step so a value persisted
  // by an older build (or hand-edited) still lands on a real notch.
  $effect(() => {
    const saved = $settings?.max_recording_time;
    if (saved == null) return;
    durationValue = DURATION_STEPS.reduce((best, s) =>
      Math.abs(s - saved) < Math.abs(best - saved) ? s : best
    );
  });

  function setDurationByIndex(index: number, save: boolean) {
    durationValue = DURATION_STEPS[Math.max(0, Math.min(LAST_INDEX, index))];
    if (save) {
      updateSetting('max_recording_time', durationValue);
    }
  }
</script>

<SettingsSection id="recording-section" title={t('recordingSettings.title')} icon="recording">
  <!-- Max Duration -->
  <div class="setting-row">
    <span class="setting-label">{t('recordingSettings.maxDuration')}</span>
    <span class="setting-value" class:caution={isLongDuration} class:danger={isDangerDuration}>
      {formatDuration(durationValue)}
    </span>
  </div>
  <SettingSlider
    value={durationIndex}
    min={0}
    max={LAST_INDEX}
    step={1}
    variant="ramp"
    rampStops={DURATION_RAMP}
    ariaLabel={t('recordingSettings.maxDurationAria')}
    labels={['5s', '30s', '1m', '3m']}
    labelPositions={[pct(5), pct(30), pct(60), pct(180)]}
    oninput={i => setDurationByIndex(i, false)}
    oncommit={i => setDurationByIndex(i, true)}
  />
  {#if isLongDuration}
    <p class="duration-caution">{t('recordingSettings.maxDurationCaution')}</p>
  {/if}

  <!-- Remove Filler Words -->
  <SettingToggle
    label={t('recordingSettings.removeFillerWords')}
    hint={t('recordingSettings.removeFillerWordsHint')}
    checked={$settings?.remove_filler_words !== false}
    onchange={v => updateSetting('remove_filler_words', v)}
  />

  <!-- Normalize Numbers to Digits -->
  <SettingToggle
    label={t('recordingSettings.normalizeNumbers')}
    hint={t('recordingSettings.normalizeNumbersHint')}
    checked={$settings?.use_numeric_digits === true}
    onchange={v => updateSetting('use_numeric_digits', v)}
  />

  <!-- Convert spoken "question mark" / "exclamation mark" to ? / ! -->
  <SettingToggle
    label={t('recordingSettings.convertSpokenPunctuation')}
    hint={t('recordingSettings.convertSpokenPunctuationHint')}
    checked={$settings?.convert_spoken_punctuation !== false}
    onchange={v => updateSetting('convert_spoken_punctuation', v)}
  />

  <!-- Append Space to Incomplete Sentences -->
  <SettingToggle
    label={t('recordingSettings.appendSpaceIncomplete')}
    hint={t('recordingSettings.appendSpaceIncompleteHint')}
    checked={$settings?.append_space_incomplete !== false}
    onchange={v => updateSetting('append_space_incomplete', v)}
  />
</SettingsSection>

<style>
  /* `.setting-row` + `.setting-label` (incl. `.locked` modifier)
		   come from app.css (shared) */

  .setting-value {
    font-size: 0.8125rem;
    font-weight: 500;
    color: #84cc16;
    transition: color 0.15s ease;
  }

  /* Match the slider ramp: yellow past 1m, red from 2m. `.danger` comes second
     so it wins when both classes are set. */
  .setting-value.caution {
    color: #eab308;
  }

  .setting-value.danger {
    color: #ef4444;
  }

  .duration-caution {
    margin: 0 0 0.5rem;
    font-size: 0.6875rem;
    line-height: 1.4;
    color: #a1a1aa;
  }
</style>
