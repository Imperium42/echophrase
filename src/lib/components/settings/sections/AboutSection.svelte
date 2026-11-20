<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * About Section
   *
   * Displays app version, update status, and links to website/docs.
   * Provides manual "Check for updates" button.
   */

  import { onMount } from 'svelte';
  import { fly } from 'svelte/transition';
  import { SettingsSection, SettingToggle } from '../primitives';
  import { isVerboseLoggingEnabled, setVerboseLoggingEnabled } from '$lib/utils/devLog';
  import { WEBSITE_URLS, SPONSORS_URL, SOURCE_CODE_URL, isDev } from '$lib/config/environment';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { docsUrl } from '$lib/utils/docsUrl';
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';
  import {
    currentVersion,
    updateStatus,
    updateVersion,
    isChecking,
    checkForUpdates,
    initUpdater,
    openReleaseNotes,
    managedUpgrade,
  } from '$lib/stores/updater';

  let verboseLogs = $state(isVerboseLoggingEnabled());

  // "Latest" / "Check failed" badge state
  let showLatest = $state(false);
  let showCheckFailed = $state(false);
  let latestTimeout: ReturnType<typeof setTimeout> | null = null;

  // Initialize updater on mount
  onMount(() => {
    initUpdater();
  });

  let commandCopied = $state(false);
  let copiedTimeout: ReturnType<typeof setTimeout> | null = null;

  async function copyUpgradeCommand() {
    const command = $managedUpgrade?.command;
    if (!command) return;
    try {
      await navigator.clipboard.writeText(command);
    } catch {
      return;
    }
    commandCopied = true;
    if (copiedTimeout) clearTimeout(copiedTimeout);
    copiedTimeout = setTimeout(() => (commandCopied = false), 1500);
  }

  async function handleCheckUpdates() {
    const hasUpdate = await checkForUpdates(false);
    if (hasUpdate === true) return;
    // null = the check itself failed (offline, bad manifest). Never claim "Latest"
    // in that case - we have no idea whether this build is current.
    showLatest = hasUpdate === false;
    showCheckFailed = hasUpdate === null;
    if (latestTimeout) clearTimeout(latestTimeout);
    latestTimeout = setTimeout(() => {
      showLatest = false;
      showCheckFailed = false;
    }, 2500);
  }

  function openWebsite() {
    launchUrl(WEBSITE_URLS.home);
  }

  function openSupport() {
    launchUrl(WEBSITE_URLS.support);
  }

  function openDocs() {
    launchUrl(docsUrl(''));
  }

  const localizedDocsUrl = $derived(docsUrl(''));

  function openDiscord() {
    launchUrl(WEBSITE_URLS.discord);
  }

  function openSourceCode() {
    launchUrl(SOURCE_CODE_URL);
  }

  function openSponsors() {
    launchUrl(SPONSORS_URL);
  }

  const ATTRIBUTION_URL = 'https://echophrase.com/attribution';
  function openLicenses() {
    launchUrl(ATTRIBUTION_URL);
  }

  /** Strip protocol, www, and any basic auth credentials for display */
  function displayUrl(url: string): string {
    return url.replace(/^https?:\/\/([^@]+@)?(www\.)?/, '');
  }

  // Easter-egg tooltip: rendered with position:fixed so it can escape the
  // settings modal's overflow:hidden / scroll container, then clamped to the
  // viewport so it never goes off-app (Tauri webview can't render outside).
  let tooltipEl: HTMLSpanElement | undefined = $state();

  function positionTooltip(e: MouseEvent) {
    if (!tooltipEl) return;
    const anchor = e.currentTarget as HTMLElement;
    const a = anchor.getBoundingClientRect();
    // Make visible (off-screen) so we can measure, then clamp.
    tooltipEl.style.visibility = 'hidden';
    tooltipEl.style.opacity = '0';
    tooltipEl.style.left = '0px';
    tooltipEl.style.top = '0px';
    const t = tooltipEl.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const margin = 8;
    let left = a.left + a.width / 2 - t.width / 2;
    left = Math.max(margin, Math.min(left, vw - t.width - margin));
    let top = a.top - t.height - 8;
    if (top < margin) top = a.bottom + 8; // flip below if no room above
    top = Math.min(top, vh - t.height - margin);
    tooltipEl.style.left = `${left}px`;
    tooltipEl.style.top = `${top}px`;
    tooltipEl.style.visibility = 'visible';
    tooltipEl.style.opacity = '1';
  }

  function hideTooltip() {
    if (!tooltipEl) return;
    tooltipEl.style.opacity = '0';
    tooltipEl.style.visibility = 'hidden';
  }
</script>

<SettingsSection title={t('about.title')} icon="info">
  <!-- Version Info -->
  <div class="about-row">
    <div class="version-info">
      <span class="version-label">{t('version.version')}</span>
      <span class="version-value">{$currentVersion || '...'}</span>
      {#if isDev}
        <span class="dev-tag">(Dev)</span>
      {/if}
      <button
        type="button"
        class="release-notes-btn"
        onclick={openReleaseNotes}
        aria-label={t('update.viewReleaseNotes')}
      >
        <Icon name="release-notes" size={14} strokeWidth={2} />
        <span class="link-tooltip">{t('update.viewReleaseNotes')}</span>
      </button>
    </div>

    <div class="update-btn-wrapper">
      {#if showLatest}
        <span class="latest-badge">{t('about.latest')}</span>
      {:else if showCheckFailed}
        <span class="latest-badge check-failed-badge" role="alert"
          >{t('about.updateCheckFailed')}</span
        >
      {/if}
      <div class="update-btn-row">
        <button class="update-btn" onclick={handleCheckUpdates} disabled={$isChecking}>
          <!-- Invisible sizer: reserves the idle-state width so swapping to
				     "Checking..." never resizes the button (grid stacks both). -->
          <span class="update-btn-sizer" aria-hidden="true">
            <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
              <path
                d="M17.65 6.35A7.958 7.958 0 0012 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"
              />
            </svg>
            {t('version.checkForUpdates')}
          </span>
          <span class="update-btn-content">
            {#if $isChecking}
              <div class="spinner"></div>
              {t('about.checking')}
            {:else if $updateStatus === 'available'}
              <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                <path d="M12 4l-1.41 1.41L16.17 11H4v2h12.17l-5.58 5.59L12 20l8-8z" />
              </svg>
              {t('about.updateTo', { version: $updateVersion || '' })}
            {:else}
              <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                <path
                  d="M17.65 6.35A7.958 7.958 0 0012 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0112 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z"
                />
              </svg>
              {t('version.checkForUpdates')}
            {/if}
          </span>
        </button>
        {#if $managedUpgrade?.command}
          <button
            type="button"
            class="copy-cmd-btn"
            transition:fly={{ x: -14, duration: 220 }}
            onclick={copyUpgradeCommand}
            title={commandCopied
              ? t('about.copied')
              : `${t('about.copyCommand')}: ${$managedUpgrade.command}`}
            aria-label={t('about.copyCommand')}
          >
            <Icon name={commandCopied ? 'check' : 'copy'} size={16} />
          </button>
        {/if}
      </div>
    </div>
  </div>

  {#if $managedUpgrade}
    <div class="managed-upgrade" role="status">
      <p>{t('about.managedUpdateAvailable', { version: $managedUpgrade.version })}</p>
      {#if $managedUpgrade.command}
        <div class="managed-upgrade-row">
          <code>{$managedUpgrade.command}</code>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Diagnostics -->
  <SettingToggle
    label={t('about.verboseLogs')}
    hint={t('about.verboseLogsHint')}
    checked={verboseLogs}
    onchange={value => {
      verboseLogs = value;
      setVerboseLoggingEnabled(value);
    }}
  />

  <!-- Quick Links -->
  <div class="about-row links-row">
    <button class="link-btn" onclick={openWebsite}>
      <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
        <path
          d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z"
        />
      </svg>
      {t('about.website')}
      <span class="link-tooltip">{displayUrl(WEBSITE_URLS.home)}</span>
    </button>
    <button class="link-btn" onclick={openSupport}>
      <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
        <path
          d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 17h-2v-2h2v2zm2.07-7.75l-.9.92C13.45 12.9 13 13.5 13 15h-2v-.5c0-1.1.45-2.1 1.17-2.83l1.24-1.26c.37-.36.59-.86.59-1.41 0-1.1-.9-2-2-2s-2 .9-2 2H8c0-2.21 1.79-4 4-4s4 1.79 4 4c0 .88-.36 1.68-.93 2.25z"
        />
      </svg>
      {t('about.support')}
      <span class="link-tooltip">{displayUrl(WEBSITE_URLS.support)}</span>
    </button>
    <button class="link-btn" onclick={openDocs}>
      <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
        <path
          d="M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zm-3-5V3.5L18.5 9H13z"
        />
      </svg>
      {t('about.docs')}
      <span class="link-tooltip">{displayUrl(localizedDocsUrl)}</span>
    </button>
    <button class="link-btn" onclick={openDiscord}>
      <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
        <path
          d="M19.27 5.33C17.94 4.71 16.5 4.26 15 4a.09.09 0 0 0-.07.03c-.18.33-.39.76-.53 1.09a16.09 16.09 0 0 0-4.8 0c-.14-.34-.36-.76-.54-1.09c-.01-.02-.04-.03-.07-.03c-1.5.26-2.93.71-4.27 1.33c-.01 0-.02.01-.03.02c-2.72 4.07-3.47 8.03-3.1 11.95c0 .02.01.04.03.05c1.8 1.32 3.53 2.12 5.24 2.65c.03.01.06 0 .07-.02c.4-.55.76-1.13 1.07-1.74c.02-.04 0-.08-.04-.09c-.57-.22-1.11-.48-1.64-.78c-.04-.02-.04-.08-.01-.11c.11-.08.22-.17.33-.25c.02-.02.05-.02.07-.01c3.44 1.57 7.15 1.57 10.55 0c.02-.01.05-.01.07.01c.11.09.22.17.33.26c.04.03.04.09-.01.11c-.52.31-1.07.56-1.64.78c-.04.01-.05.06-.04.09c.32.61.68 1.19 1.07 1.74c.03.01.06.02.09.01c1.72-.53 3.45-1.33 5.25-2.65c.02-.01.03-.03.03-.05c.44-4.53-.73-8.46-3.1-11.95c-.01-.01-.02-.02-.04-.02zM8.52 14.91c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12c0 1.17-.84 2.12-1.89 2.12zm6.97 0c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12c0 1.17-.83 2.12-1.89 2.12z"
        />
      </svg>
      {t('about.discord')}
      <span class="link-tooltip">{displayUrl(WEBSITE_URLS.discord)}</span>
    </button>
    <button class="link-btn" onclick={openSourceCode}>
      <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true">
        <path
          d="M12 .5C5.65.5.5 5.65.5 12a11.5 11.5 0 0 0 7.86 10.92c.58.1.79-.25.79-.56v-2c-3.2.7-3.88-1.37-3.88-1.37-.52-1.33-1.28-1.68-1.28-1.68-1.05-.72.08-.7.08-.7 1.16.08 1.77 1.19 1.77 1.19 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.55-.29-5.24-1.28-5.24-5.68 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.17 1.18a11 11 0 0 1 5.77 0c2.2-1.49 3.17-1.18 3.17-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.83 1.19 3.09 0 4.41-2.69 5.39-5.25 5.67.41.36.78 1.06.78 2.14v3.17c0 .31.21.67.8.56A11.5 11.5 0 0 0 23.5 12C23.5 5.65 18.35.5 12 .5z"
        />
      </svg>
      {t('about.sourceCode')}
      <span class="link-tooltip">{displayUrl(SOURCE_CODE_URL)}</span>
    </button>
  </div>

  <!-- Support Us (GitHub Sponsors) -->
  <div class="about-row sponsor-row">
    <span class="sponsor-hint">{t('about.supportUsHint')}</span>
    <button class="link-btn sponsor-btn" onclick={openSponsors}>
      <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true">
        <path
          d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z"
        />
      </svg>
      {t('about.supportUs')}
      <span class="link-tooltip">{displayUrl(SPONSORS_URL)}</span>
    </button>
  </div>

  <!-- Copyright -->
  <div class="copyright">
    © 2025-2026 <span
      class="easter-egg"
      onmouseenter={positionTooltip}
      onmouseleave={hideTooltip}
      onclick={() => launchUrl(WEBSITE_URLS.imperium)}
      onkeydown={e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          launchUrl(WEBSITE_URLS.imperium);
        }
      }}
      role="link"
      tabindex="0"
      title={WEBSITE_URLS.imperium}
      >Imperium42<span class="snowball-tooltip" bind:this={tooltipEl}>{t('about.easterEgg')}</span
      ></span
    >
    LLC. {t('about.copyright')}
    <span class="copyright-divider">·</span>
    <button class="text-link" onclick={openLicenses} title={ATTRIBUTION_URL}
      >{t('about.licenses')}</button
    >
    <div class="trademark">{t('about.trademark')}</div>
  </div>
</SettingsSection>

<style>
  .about-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
  }

  .version-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .version-label {
    font-size: 0.8125rem;
    color: #71717a;
  }

  .version-value {
    font-size: 0.8125rem;
    font-weight: 500;
    color: #e4e4e7;
    font-family: ui-monospace, 'SF Mono', monospace;
    white-space: nowrap;
  }

  .dev-tag {
    font-size: 0.75rem;
    font-weight: 600;
    color: #fb923c;
    font-family: ui-monospace, 'SF Mono', monospace;
  }

  .release-notes-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.25rem;
    margin-left: 0.125rem;
    color: #71717a;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    cursor: pointer;
    transition:
      color 0.15s,
      background 0.15s;
  }

  .release-notes-btn:hover {
    color: #e4e4e7;
    background: rgba(255, 255, 255, 0.06);
  }

  .update-btn-wrapper {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
  }

  .latest-badge {
    position: absolute;
    bottom: 100%;
    margin-bottom: 0.25rem;
    font-size: 0.625rem;
    font-weight: 600;
    color: #4ade80;
    animation: fade-in-out 2.5s ease-in-out forwards;
  }

  .check-failed-badge {
    color: #f87171;
    white-space: nowrap;
  }

  @keyframes fade-in-out {
    0% {
      opacity: 0;
      transform: translateY(4px);
    }
    15% {
      opacity: 1;
      transform: translateY(0);
    }
    85% {
      opacity: 1;
      transform: translateY(0);
    }
    100% {
      opacity: 0;
      transform: translateY(-4px);
    }
  }

  .update-btn {
    /* Grid stacks the invisible sizer and the live content in the same cell,
		   so the button is always as wide as the widest of the two - no resize
		   when the label swaps to "Checking..." */
    display: grid;
    padding: 0.5rem 0.875rem;
    background: rgba(139, 92, 246, 0.1);
    border: 1px solid rgba(139, 92, 246, 0.2);
    border-radius: 0.375rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: #c4b5fd;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .update-btn:hover:not(:disabled) {
    background: rgba(139, 92, 246, 0.2);
    border-color: rgba(139, 92, 246, 0.3);
  }

  .update-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .update-btn-sizer,
  .update-btn-content {
    grid-area: 1 / 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    white-space: nowrap;
  }

  .update-btn-sizer {
    visibility: hidden;
  }

  .links-row {
    justify-content: space-between;
    gap: 0.75rem;
  }

  .link-btn {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.375rem 0.625rem;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.25rem;
    font-size: 0.75rem;
    font-weight: 450;
    color: #a1a1aa;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .link-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.15);
    color: #e4e4e7;
  }

  .sponsor-hint {
    font-size: 0.8125rem;
    color: #a1a1aa;
  }

  .sponsor-btn {
    flex-shrink: 0;
    color: #f9a8d4;
    border-color: rgba(236, 72, 153, 0.35);
  }

  .sponsor-btn:hover {
    color: #fbcfe8;
    background: rgba(236, 72, 153, 0.1);
    border-color: rgba(236, 72, 153, 0.55);
  }

  .link-tooltip {
    position: absolute;
    bottom: calc(100% + 8px);
    left: 0;
    transform: translateY(4px);
    padding: 0.3rem 0.5rem;
    background: #1e1e2e;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.375rem;
    font-size: 0.625rem;
    font-family: ui-monospace, 'SF Mono', monospace;
    color: #a1a1aa;
    white-space: nowrap;
    pointer-events: none;
    opacity: 0;
    transition:
      opacity 0.15s ease,
      transform 0.15s ease;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    z-index: 100;
  }

  .link-tooltip::after {
    content: '';
    position: absolute;
    top: 100%;
    left: 1rem;
    border: 4px solid transparent;
    border-top-color: #1e1e2e;
  }

  .link-btn:hover .link-tooltip,
  .release-notes-btn:hover .link-tooltip {
    opacity: 1;
    transform: translateY(0);
  }

  .copyright {
    padding-top: 0.75rem;
    font-size: 0.6875rem;
    color: #52525b;
    text-align: center;
  }

  .trademark {
    margin-top: 0.25rem;
    font-size: 0.625rem;
    color: #3f3f46;
  }

  .copyright-divider {
    margin: 0 0.375rem;
    color: #3f3f46;
  }

  .text-link {
    background: none;
    border: none;
    padding: 0;
    font-size: inherit;
    color: #71717a;
    cursor: pointer;
    text-decoration: underline;
    text-decoration-style: dotted;
    text-underline-offset: 2px;
    transition: color 0.15s ease;
  }

  .text-link:hover {
    color: #a1a1aa;
  }

  .easter-egg {
    position: relative;
    text-decoration: underline;
    text-decoration-style: dotted;
    text-underline-offset: 2px;
    cursor: pointer;
    color: #71717a;
    transition: color 0.15s ease;
  }

  .easter-egg:hover {
    color: #a1a1aa;
  }

  .snowball-tooltip {
    position: fixed;
    left: 0;
    top: 0;
    width: max-content;
    max-width: 280px;
    padding: 0.5rem 0.75rem;
    background: #1e1e2e;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 0.5rem;
    font-size: 0.6875rem;
    line-height: 1.4;
    color: #d4d4d8;
    text-align: left;
    white-space: normal;
    pointer-events: none;
    opacity: 0;
    visibility: hidden;
    transition: opacity 0.2s ease;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    z-index: 9999;
  }

  .spinner {
    width: 0.875rem;
    height: 0.875rem;
    border: 1.5px solid rgba(196, 181, 253, 0.2);
    border-top-color: #c4b5fd;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .managed-upgrade {
    margin-top: 0.75rem;
    padding: 0.75rem 1rem;
    border-radius: 0.75rem;
    border: 1px solid rgba(139, 92, 246, 0.3);
    background: rgba(139, 92, 246, 0.08);
    font-size: 0.8125rem;
    color: rgb(212, 212, 216);
  }

  .managed-upgrade p {
    margin: 0 0 0.5rem 0;
  }

  .managed-upgrade-row {
    display: flex;
    align-items: center;
  }

  .update-btn-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .copy-cmd-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    background: rgba(139, 92, 246, 0.1);
    border: 1px solid rgba(139, 92, 246, 0.2);
    border-radius: 0.375rem;
    color: #c4b5fd;
    cursor: pointer;
    transition:
      background 0.15s ease,
      border-color 0.15s ease;
  }

  .copy-cmd-btn:hover {
    background: rgba(139, 92, 246, 0.2);
    border-color: rgba(139, 92, 246, 0.4);
  }

  .copy-cmd-btn:focus-visible {
    outline: 2px solid rgb(34, 211, 238);
    outline-offset: 2px;
  }

  .managed-upgrade code {
    font-family: ui-monospace, monospace;
    color: rgb(196, 181, 253);
    overflow-wrap: anywhere;
  }
</style>
