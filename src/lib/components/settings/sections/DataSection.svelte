<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Data Section
   *
   * Privacy and telemetry settings plus legal links. Everything here is
   * local: there is no account or server-side data to export or delete.
   */

  import { SettingsSection, SettingToggle } from '../primitives';
  import { getSettingsContext } from '../context';
  import { WEBSITE_URLS } from '$lib/config/environment';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { docsUrl } from '$lib/utils/docsUrl';
  import { t } from '$lib/locales';
  import { isOfficialBuild } from '$lib/ipc/telemetry';

  const { settings, updateSetting } = getSettingsContext();

  /** Public per-event telemetry disclosure (also links out to the privacy policy). */
  const telemetryDocsUrl = $derived(docsUrl('/privacy/telemetry'));

  /** Community builds have no telemetry compiled in, so there's nothing to toggle. */
  let officialBuild = $state(false);
  isOfficialBuild().then(v => (officialBuild = v));
</script>

<SettingsSection title={t('data.title')} icon="data" id="data-section">
  <!-- No confirm-on-opt-out: arguing with a decision the user already made is a
	     dark pattern. The 📖 button opens the public per-event disclosure page. -->
  {#if officialBuild}
    <SettingToggle
      label={t('data.helpImprove')}
      hint={t('data.helpImproveHint')}
      docsUrl={telemetryDocsUrl}
      docsTooltip={t('data.telemetryDocsTooltip')}
      checked={$settings?.telemetry_enabled ?? true}
      onchange={v => updateSetting('telemetry_enabled', v)}
    />
  {/if}

  <div class="legal-links">
    <button class="legal-link" onclick={() => launchUrl(WEBSITE_URLS.terms)}>
      {t('data.termsOfService')}
    </button>
    <span class="separator">·</span>
    <button class="legal-link" onclick={() => launchUrl(WEBSITE_URLS.privacy)}>
      {t('data.privacyPolicy')}
    </button>
  </div>
</SettingsSection>

<style>
  /* ── Legal links ── */
  .legal-links {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding-top: 0.75rem;
    margin-top: 0.5rem;
    border-top: 1px solid rgba(63, 63, 70, 0.5);
  }

  .legal-link {
    background: none;
    border: none;
    padding: 0;
    font-size: 0.6875rem;
    color: #71717a;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .legal-link:hover {
    color: #a1a1aa;
  }

  .separator {
    color: #52525b;
    font-size: 0.625rem;
  }
</style>
