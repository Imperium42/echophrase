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

  import { SettingsSection, SettingToggle } from '../primitives';
  import { getSettingsContext } from '../context';
  import { t } from '$lib/locales';

  const { settings, updateSetting } = getSettingsContext();

  // Keep these in step with MIN/MAX_IDLE_UNLOAD_MINUTES in transcription/idle.rs.
  const MIN_MINUTES = 1;
  const MAX_MINUTES = 120;
  const DEFAULT_MINUTES = 10;

  const enabled = $derived($settings?.idle_unload_enabled ?? false);

  // Local draft so typing (including an empty field mid-edit) never writes junk;
  // committed on change/blur only.
  let minutesDraft = $state(String(DEFAULT_MINUTES));
  $effect(() => {
    minutesDraft = String($settings?.idle_unload_minutes ?? DEFAULT_MINUTES);
  });

  function commitMinutes() {
    const parsed = Math.round(Number(minutesDraft));
    const next = Number.isFinite(parsed)
      ? Math.min(MAX_MINUTES, Math.max(MIN_MINUTES, parsed))
      : ($settings?.idle_unload_minutes ?? DEFAULT_MINUTES);
    minutesDraft = String(next);
    if (next !== ($settings?.idle_unload_minutes ?? DEFAULT_MINUTES)) {
      updateSetting('idle_unload_minutes', next);
    }
  }
</script>

{#snippet afterLabel()}
  <span class="idle-prefix" class:dim={!enabled}>{t('optimization.idleUnloadPrefix')}</span>
  <input
    type="number"
    class="idle-minutes"
    min={MIN_MINUTES}
    max={MAX_MINUTES}
    step="1"
    inputmode="numeric"
    bind:value={minutesDraft}
    onchange={commitMinutes}
    onblur={commitMinutes}
    disabled={!enabled}
    aria-label={t('optimization.idleUnloadMinutesAria')}
  />
  <span class="idle-suffix" class:dim={!enabled}>{t('optimization.idleUnloadSuffix')}</span>
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

<style>
  .idle-minutes {
    width: 3rem;
    padding: 0.25rem 0.375rem;
    appearance: textfield;
    -moz-appearance: textfield;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.375rem;
    color: #e4e4e7;
    font-size: 0.8125rem;
    text-align: center;
  }

  /* Hide the browser's native spinner - it renders as a bright block on the dark theme. */
  .idle-minutes::-webkit-inner-spin-button,
  .idle-minutes::-webkit-outer-spin-button {
    -webkit-appearance: none;
    appearance: none;
    margin: 0;
  }

  .idle-minutes:focus-visible {
    outline: 2px solid rgba(132, 204, 22, 0.6);
    outline-offset: 1px;
  }

  .idle-minutes:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .idle-prefix,
  .idle-suffix {
    font-size: 0.8125rem;
    color: #d4d4d8;
  }

  .idle-prefix.dim,
  .idle-suffix.dim {
    color: #71717a;
  }
</style>
