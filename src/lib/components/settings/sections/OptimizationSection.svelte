<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Optimization Section
   *
   * Memory-saving options. Currently one opt-in: unload the speech model after
   * N idle minutes. Off by default because the next recording has to reload the
   * model (about 7 s warm). Backed by `transcription::idle` in the Rust backend,
   * which re-reads these settings on every pass, so no restart is needed.
   */

  import { SettingsSection, SettingToggle, SettingNumberInput } from '../primitives';
  import { getSettingsContext } from '../context';
  import { t } from '$lib/locales';

  const { settings, updateSetting } = getSettingsContext();

  // Keep these in step with MIN/MAX_IDLE_UNLOAD_MINUTES in transcription/idle.rs.
  const MIN_MINUTES = 1;
  const MAX_MINUTES = 120;
  const DEFAULT_MINUTES = 10;

  const enabled = $derived($settings?.idle_unload_enabled ?? false);
</script>

{#snippet afterLabel()}
  <SettingNumberInput
    value={$settings?.idle_unload_minutes}
    fallback={DEFAULT_MINUTES}
    min={MIN_MINUTES}
    max={MAX_MINUTES}
    prefix={t('optimization.idleUnloadPrefix')}
    suffix={t('optimization.idleUnloadSuffix')}
    disabled={!enabled}
    ariaLabel={t('optimization.idleUnloadMinutesAria')}
    oncommit={v => updateSetting('idle_unload_minutes', v)}
  />
{/snippet}

<SettingsSection id="optimization-section" title={t('optimization.title')} icon="optimization">
  <SettingToggle
    label={t('optimization.idleUnloadLabel')}
    hint={t('optimization.idleUnloadHint')}
    checked={enabled}
    onchange={v => updateSetting('idle_unload_enabled', v)}
    {afterLabel}
  />
</SettingsSection>
