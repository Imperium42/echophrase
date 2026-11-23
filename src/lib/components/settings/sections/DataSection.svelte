<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Data Section
   *
   * Legal links. Echophrase collects nothing: there is no telemetry, no
   * account and no server-side data to export or delete.
   */

  import { SettingsSection } from '../primitives';
  import { WEBSITE_URLS } from '$lib/config/environment';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { t } from '$lib/locales';
</script>

<SettingsSection title={t('data.title')} icon="data" id="data-section">
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
