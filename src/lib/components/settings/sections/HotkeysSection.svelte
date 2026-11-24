<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Hotkeys Section
   *
   * Global hotkey configuration for toggle recording.
   */

  import { SettingsSection } from '../primitives';
  import { getSettingsContext } from '../context';
  import { hotkeyStore } from '$lib/stores/hotkeys';
  import { lastError as hotkeyRegisterError } from '$lib/services/keyboardService';
  import { hotkeyAliases, HOTKEY_ALIAS_MAX_LEN } from '$lib/stores/hotkeyAliases';
  import { platformCache } from '$lib/stores/platformCache';
  import { formatHotkey, formatKeyToken, tokensToDisplay } from '$lib/utils/hotkeyFormatter';
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';

  const { settings } = getSettingsContext();

  let isCapturingHotkey = $state(false);
  let capturedKeys = $state<string[]>([]);
  let isEditingAliases = $state(false);
  const isWSL = platformCache.isWsl;

  /**
   * Aliasable tokens = the non-modifier keys of the current binding. Modifiers
   * are excluded because Ctrl/Alt/Shift/Cmd already have universal labels; the
   * point of this feature is naming the unlabelled keys (F15, mouse buttons).
   *
   * Depends on $hotkeyAliases so the rendered labels update as the user types.
   */
  const MODIFIER_TOKENS = new Set([
    'ctrl',
    'control',
    'meta',
    'cmd',
    'command',
    'super',
    'alt',
    'option',
    'shift',
  ]);

  const aliasableKeys = $derived(
    $hotkeyStore.currentHotkey
      .split('+')
      .map(token => token.trim().toLowerCase())
      .filter(token => token && !MODIFIER_TOKENS.has(token))
  );

  // Pass $hotkeyAliases explicitly so renaming a key re-renders this label.
  const hotkeyLabel = $derived(
    formatHotkey($hotkeyStore.currentHotkey, { aliases: $hotkeyAliases })
  );

  function startHotkeyCapture() {
    isEditingAliases = false;
    isCapturingHotkey = true;
    capturedKeys = [];
    window.addEventListener('keydown', handleKeyCapture);
    window.addEventListener('keyup', handleKeyRelease);
  }

  function handleKeyCapture(event: KeyboardEvent) {
    if (!isCapturingHotkey) return;
    event.preventDefault();
    event.stopPropagation();

    if (event.key === 'Escape') {
      cancelHotkeyCapture();
      return;
    }

    // Build canonical lowercase tokens (hotkeys-js format). The primary modifier
    // is stored as `ctrl` for both Ctrl and the macOS Command key: convertToAccelerator
    // maps `ctrl` -> CommandOrControl, which fires Cmd on macOS / Ctrl elsewhere.
    const keys: string[] = [];
    if (event.ctrlKey || event.metaKey) keys.push('ctrl');
    if (event.altKey) keys.push('alt');
    if (event.shiftKey) keys.push('shift');

    if (!['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) {
      // event.key for the spacebar is ' ' - normalize to the `space` token.
      const raw = event.key === ' ' ? 'Space' : event.key;
      keys.push(raw.toLowerCase());
    }
    capturedKeys = keys;
  }

  function handleKeyRelease(_event: KeyboardEvent) {
    if (!isCapturingHotkey || capturedKeys.length === 0) return;
    if (capturedKeys.length >= 2) {
      const hotkeyString = capturedKeys.join('+');
      hotkeyStore.setHotkey(hotkeyString);
      cancelHotkeyCapture();
    }
  }

  function cancelHotkeyCapture() {
    isCapturingHotkey = false;
    capturedKeys = [];
    window.removeEventListener('keydown', handleKeyCapture);
    window.removeEventListener('keyup', handleKeyRelease);
  }
</script>

<SettingsSection title={t('hotkeys.title')} icon="hotkeys">
  {#if $settings?.show_hotkey_changer !== false}
    <div class="hotkey-row">
      <div class="hotkey-info">
        <span class="hotkey-action">{t('hotkeys.toggleRecording')}</span>
        <span class="hotkey-hint">{t('hotkeys.globalHint')}</span>
        <span class="hotkey-hint-alt">{t('hotkeys.focusHint')}</span>
      </div>
      {#if isCapturingHotkey}
        <div class="hotkey-capture">
          <span class="hotkey-keys"
            >{capturedKeys.length > 0
              ? tokensToDisplay(capturedKeys)
              : t('hotkeys.pressKeys')}</span
          >
          <button
            class="hotkey-cancel"
            onclick={cancelHotkeyCapture}
            aria-label="Cancel hotkey capture"
          >
            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>
      {:else}
        <div class="hotkey-controls">
          <button
            class="hotkey-button"
            onclick={startHotkeyCapture}
            title={t('hotkeys.changeHotkey')}
            aria-label="{t('hotkeys.changeHotkey')}, currently {hotkeyLabel}"
          >
            {hotkeyLabel}
          </button>
          {#if aliasableKeys.length > 0}
            <button
              class="alias-button"
              class:active={isEditingAliases}
              onclick={() => (isEditingAliases = !isEditingAliases)}
              title={t('hotkeys.alias')}
              aria-label={t('hotkeys.alias')}
              aria-expanded={isEditingAliases}
            >
              <Icon name="alias" size={12} />
            </button>
          {/if}
        </div>
      {/if}
    </div>
    {#if $hotkeyRegisterError && !isCapturingHotkey}
      <p class="register-error" role="alert" title={$hotkeyRegisterError}>
        <Icon name="warning" size={14} class="inline align-text-bottom" />
        {t('hotkeys.registerFailed')}
      </p>
    {/if}
    {#if isEditingAliases && !isCapturingHotkey}
      <div class="alias-editor">
        <p class="alias-hint">{t('hotkeys.aliasHint')}</p>
        {#each aliasableKeys as key (key)}
          <label class="alias-row">
            <span class="alias-key">{formatKeyToken(key, { ignoreAlias: true })}</span>
            <input
              class="alias-input"
              type="text"
              maxlength={HOTKEY_ALIAS_MAX_LEN}
              placeholder={t('hotkeys.aliasPlaceholder')}
              value={$hotkeyAliases[key] ?? ''}
              oninput={e => hotkeyAliases.setAlias(key, e.currentTarget.value)}
            />
          </label>
        {/each}
      </div>
    {/if}
    {#if isWSL}
      <p class="wsl-warning">
        <Icon name="warning" size={14} class="inline align-text-bottom" />
        {t('hotkeys.wslWarning')}
      </p>
    {/if}
  {:else}
    <p class="hotkey-hint-static">
      {t('hotkeys.hotkeyStatic', { hotkey: hotkeyLabel })}
    </p>
  {/if}
</SettingsSection>

<style>
  .hotkey-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.5rem 0;
  }

  .hotkey-info {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .hotkey-action {
    font-size: 0.8125rem;
    color: #d4d4d8;
  }

  .hotkey-hint {
    font-size: 0.6875rem;
    color: #52525b;
  }

  .hotkey-hint-alt {
    font-size: 0.625rem;
    color: #3f3f46;
    font-style: italic;
  }

  .hotkey-hint-static {
    font-size: 0.6875rem;
    color: #52525b;
  }

  .hotkey-button {
    padding: 0.375rem 0.625rem;
    background: rgba(255, 255, 255, 0.04);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    font-weight: 500;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    color: #71717a;
    cursor: pointer;
    /* Specific properties only - avoid "all" which causes repaint flicker */
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .hotkey-button:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #a1a1aa;
    /* No transform - avoids GPU layer creation/destruction flicker */
  }

  .hotkey-button:active {
    background: rgba(255, 255, 255, 0.06);
  }

  .hotkey-controls {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  /* Deliberately small - a secondary affordance that must not compete with
     the hotkey pill it sits beside. */
  .alias-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0.25rem;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    color: #52525b;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .alias-button:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #a1a1aa;
  }

  .alias-button.active {
    background: rgba(255, 255, 255, 0.08);
    color: #a78bfa;
  }

  .alias-editor {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding: 0.5rem 0 0.25rem;
  }

  .alias-hint {
    font-size: 0.625rem;
    color: #52525b;
    margin: 0;
  }

  .alias-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }

  .alias-key {
    font-size: 0.6875rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    color: #71717a;
  }

  .alias-input {
    width: 7rem;
    padding: 0.25rem 0.5rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    color: #d4d4d8;
    outline: none;
    transition: border-color 0.15s ease;
  }

  .alias-input:focus {
    border-color: rgba(167, 139, 250, 0.5);
  }

  .hotkey-capture {
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  .hotkey-keys {
    padding: 0.375rem 0.625rem;
    background: rgba(132, 204, 22, 0.08);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.6875rem;
    font-weight: 500;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    color: #84cc16;
  }

  .hotkey-cancel {
    padding: 0.25rem;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .hotkey-cancel:hover {
    background: rgba(239, 68, 68, 0.1);
    color: #ef4444;
  }

  .wsl-warning {
    font-size: 0.6875rem;
    color: #fbbf24;
    margin-top: 0.375rem;
    opacity: 0.8;
  }

  .register-error {
    font-size: 0.6875rem;
    color: #f87171;
    margin-top: 0.375rem;
  }
</style>
