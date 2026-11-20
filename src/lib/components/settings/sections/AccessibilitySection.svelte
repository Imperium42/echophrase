<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Accessibility Section
   *
   * Cross-platform UI scaling: Auto (use the system default size) or Manual
   * (50%-200% slider), plus an opt-in "scale UI with window size" mode.
   * Applied live by useUiScale via CSS zoom on :root - no restart needed.
   */

  import { SettingsSection, SettingToggle, SettingSlider } from '../primitives';
  import SettingRow from '../primitives/SettingRow.svelte';
  import { getSettingsContext } from '../context';
  import { uiScaleEffective, UI_SCALE_MIN, UI_SCALE_MAX } from '$lib/composables/useUiScale';
  import { resetWindowSize } from '$lib/utils/windowSize';
  import { get } from 'svelte/store';
  import { tick } from 'svelte';
  import { t } from '$lib/locales';

  const { settings, updateSetting } = getSettingsContext();

  const mode = $derived($settings?.ui_scale_mode === 'manual' ? 'manual' : 'auto');

  // Slider works in whole percent (50-200, step 5); settings store the factor
  let sliderPercent = $state(100);
  $effect(() => {
    sliderPercent = Math.round(($settings?.ui_scale_manual ?? 1.0) * 100);
  });

  function onSliderChange(percent: number) {
    sliderPercent = percent;
    const factor = Math.min(UI_SCALE_MAX, Math.max(UI_SCALE_MIN, percent / 100));
    updateSetting('ui_scale_manual', factor);
  }

  function setMode(next: 'auto' | 'manual') {
    updateSetting('ui_scale_mode', next);
  }

  // Reset scale to defaults (Auto / 100% manual / no window-scaling) and
  // snap the window back to the default size for the resulting zoom.
  async function resetScale() {
    await updateSetting('ui_scale_mode', 'auto');
    await updateSetting('ui_scale_manual', 1.0);
    await updateSetting('ui_scale_with_window', false);
    await tick();
    await resetWindowSize(get(uiScaleEffective));
  }
</script>

<SettingsSection title={t('accessibility.title')} icon="accessibility">
  <SettingRow label={t('accessibility.uiScale')} hint={t('accessibility.uiScaleHint')}>
    <div class="mode-toggle" role="radiogroup" aria-label={t('accessibility.uiScale')}>
      <button
        type="button"
        class="mode-btn"
        class:active={mode === 'auto'}
        role="radio"
        aria-checked={mode === 'auto'}
        onclick={() => setMode('auto')}
      >
        {t('accessibility.autoMode')}
      </button>
      <button
        type="button"
        class="mode-btn"
        class:active={mode === 'manual'}
        role="radio"
        aria-checked={mode === 'manual'}
        onclick={() => setMode('manual')}
      >
        {t('accessibility.manualMode')}
      </button>
    </div>
  </SettingRow>

  {#if mode === 'auto'}
    <p class="mode-hint">{t('accessibility.autoModeHint')}</p>
  {:else}
    <div class="scale-slider">
      <SettingSlider
        min={50}
        max={200}
        step={5}
        value={sliderPercent}
        ariaLabel={t('accessibility.uiScale')}
        labels={['50%', '100%', '150%', '200%']}
        oninput={v => onSliderChange(v)}
        oncommit={v => onSliderChange(v)}
      />
      <span class="scale-value">{sliderPercent}%</span>
    </div>
  {/if}

  <SettingToggle
    label={t('accessibility.scaleWithWindow')}
    hint={t('accessibility.scaleWithWindowHint')}
    checked={$settings?.ui_scale_with_window ?? false}
    onchange={v => updateSetting('ui_scale_with_window', v)}
  />

  <SettingRow label={t('accessibility.reset')} hint={t('accessibility.resetHint')}>
    <button type="button" class="reset-btn" onclick={resetScale}>
      {t('accessibility.resetButton')}
    </button>
  </SettingRow>
</SettingsSection>

<style>
  .mode-toggle {
    display: flex;
    gap: 0.25rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.5rem;
    padding: 0.125rem;
  }

  .mode-btn {
    background: transparent;
    color: #a1a1aa;
    border: none;
    border-radius: 0.375rem;
    padding: 0.25rem 0.75rem;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .mode-btn:hover {
    color: #e4e4e7;
  }

  .mode-btn.active {
    background: rgba(132, 204, 22, 0.12);
    color: #84cc16;
  }

  .mode-hint {
    font-size: 0.75rem;
    color: #71717a;
    margin: 0.25rem 0 0.5rem;
  }

  .scale-slider {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding: 0.5rem 0 0.75rem;
  }

  .reset-btn {
    background: rgba(255, 255, 255, 0.06);
    color: #e4e4e7;
    padding: 0.375rem 0.875rem;
    border-radius: 0.5rem;
    font-size: 0.8125rem;
    font-weight: 500;
    border: 1px solid rgba(255, 255, 255, 0.08);
    cursor: pointer;
    transition:
      background 0.15s ease,
      border-color 0.15s ease;
  }
  .reset-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.14);
  }
  .reset-btn:active {
    filter: brightness(0.92);
  }

  .scale-value {
    min-width: 3rem;
    text-align: right;
    font-size: 0.8125rem;
    font-weight: 500;
    color: #84cc16;
    font-variant-numeric: tabular-nums;
  }
</style>
