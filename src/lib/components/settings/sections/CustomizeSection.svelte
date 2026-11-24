<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Customize Section
   *
   * UI customization toggles (show/hide various elements).
   */

  import { SettingsSection, SettingToggle } from '../primitives';
  import { getSettingsContext } from '../context';
  import { t } from '$lib/locales';

  // Window-size reset moved to the Accessibility section (its Reset also
  // restores the default window size scaled by the current UI zoom).

  const { settings, updateSetting } = getSettingsContext();

  // Declarative toggle definitions with helpful hints and defaults
  const customizeToggles = $derived([
    {
      key: 'show_hotkey_changer' as const,
      label: t('customize.showHotkeyChanger'),
      hint: t('customize.showHotkeyChangerHint'),
      defaultValue: true,
    },
    {
      key: 'show_realtime_preview' as const,
      label: t('customize.showTranscriptionPreview'),
      hint: t('customize.showTranscriptionPreviewHint'),
      defaultValue: true,
    },
    {
      key: 'show_output_confirmations' as const,
      label: t('customize.showOutputConfirmations'),
      hint: t('customize.showOutputConfirmationsHint'),
      defaultValue: true,
    },
    {
      key: 'show_copy_last_button' as const,
      label: t('customize.showCopyLastButton'),
      hint: t('customize.showCopyLastButtonHint'),
      defaultValue: true,
    },
    {
      key: 'show_input_changer' as const,
      label: t('customize.showInputChanger'),
      hint: t('customize.showInputChangerHint'),
      defaultValue: true,
    },
    {
      key: 'show_model_switcher' as const,
      label: t('customize.showModelSwitcher'),
      hint: t('customize.showModelSwitcherHint'),
      defaultValue: true,
    },
    {
      key: 'show_transcription_speed' as const,
      label: t('customize.showTranscriptionSpeed'),
      hint: t('customize.showTranscriptionSpeedHint'),
      defaultValue: false,
    },
    {
      key: 'show_last_on_hover' as const,
      label: t('customize.showLastOnHover'),
      hint: t('customize.showLastOnHoverHint'),
      defaultValue: true,
    },
  ]);
</script>

<SettingsSection title={t('customize.title')} icon="customize">
  {#each customizeToggles as toggle (toggle.key)}
    <SettingToggle
      label={toggle.label}
      hint={toggle.hint}
      checked={$settings?.[toggle.key] ?? toggle.defaultValue}
      onchange={v => updateSetting(toggle.key, v)}
    />
  {/each}

  <!-- Minimize to tray on close -->
  <SettingToggle
    label={t('customize.minimizeToTrayOnClose')}
    hint={t('customize.minimizeToTrayOnCloseHint')}
    checked={$settings?.minimize_to_tray_on_close ?? true}
    onchange={v => updateSetting('minimize_to_tray_on_close', v)}
  />
</SettingsSection>
