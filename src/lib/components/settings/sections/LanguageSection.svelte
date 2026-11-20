<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Language Section
   *
   * App display language, as a single dropdown row. Each option shows the
   * language's native name, its name in the current UI language when that
   * differs, and a BETA marker for incomplete translations.
   */

  import { SettingsSection } from '../primitives';
  import { locale, setLocale, t } from '$lib/locales';
  import { LOCALES, isValidLocale } from '$lib/locales';

  function handleChange(event: Event): void {
    const value = (event.currentTarget as HTMLSelectElement).value;
    if (isValidLocale(value)) setLocale(value);
  }

  function optionLabel(code: string, name: string, beta: boolean | undefined): string {
    const localized = t(`languageSettings.${code}`);
    const parts = [name];
    if (localized !== name) parts.push(`(${localized})`);
    if (beta) parts.push(`- ${t('languageSettings.beta')}`);
    return parts.join(' ');
  }
</script>

<SettingsSection
  id="language"
  title={t('languageSettings.title')}
  icon="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9"
>
  <div class="setting-row">
    <span class="setting-label">{t('languageSettings.title')}</span>
    <div class="select-wrapper">
      <select class="select-field" value={locale.current} onchange={handleChange}>
        {#each LOCALES as info (info.code)}
          <option value={info.code}>{optionLabel(info.code, info.name, info.beta)}</option>
        {/each}
      </select>
      <svg class="select-arrow" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"
        ></path>
      </svg>
    </div>
  </div>
</SettingsSection>

<style>
  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
  }

  .setting-label {
    font-size: 0.8125rem;
    color: #d4d4d8;
  }

  .select-wrapper {
    position: relative;
    flex: 1;
    min-width: 140px;
    max-width: 220px;
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

  .select-field:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #d4d4d8;
  }

  .select-field:focus {
    outline: none;
    background: rgba(255, 255, 255, 0.08);
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
</style>
