<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Output Section
   *
   * Output format selection, post-record action toggles,
   * and JSON metadata template editor.
   */

  import { mustInvoke } from '$lib/ipc/invoke';
  import { cmdOpenDirectory } from '$lib/ipc/commands';
  import { SettingsSection, SettingToggle } from '../primitives';
  import { getSettingsContext } from '../context';
  import { getAvailableOutputFormats, getAvailableOutputModes } from '$lib/types/settings';
  import { t } from '$lib/locales';
  import { isWindows, isMac, isLinux } from '$lib/utils/platform';
  import { truncatePath } from './output/outputActions';
  import JsonTemplateEditor from './output/JsonTemplateEditor.svelte';

  const { settings, updateSetting } = getSettingsContext();

  // Save directory state
  let isPickingDirectory = $state(false);
  let dirSaved = $state(false);
  let dirError = $state('');

  async function pickSaveDirectory() {
    if (isPickingDirectory) return;
    isPickingDirectory = true;
    dirSaved = false;
    dirError = '';
    try {
      const dir = await mustInvoke<string | null>('pick_directory');
      if (dir) {
        await updateSetting('save_directory', dir);
        dirSaved = true;
        setTimeout(() => (dirSaved = false), 2000);
      }
    } catch (e) {
      console.error('Failed to pick directory:', e);
      dirError = t('outputModes.dirFailed');
      setTimeout(() => (dirError = ''), 3000);
    } finally {
      isPickingDirectory = false;
    }
  }

  async function clearSaveDirectory() {
    await updateSetting('save_directory', null);
  }

  // Derived values for output toggles
  const currentOutputTo = $derived(
    Array.isArray($settings?.output_to)
      ? $settings.output_to
      : $settings?.output_to
        ? [$settings.output_to]
        : []
  );

  // Paste at Cursor requires clipboard (it reads from clipboard to paste).
  const pasteRequiresClipboard = $derived(currentOutputTo.includes('paste_at_cursor'));

  // Any output other than the clipboard itself?
  const hasUsableOtherOutput = $derived(currentOutputTo.some(v => v !== 'copy_to_clipboard'));

  // Get platform-specific tooltip for paste_at_cursor
  function getPasteAtCursorTip(): string {
    if (isWindows()) return t('outputSettings.pasteAtCursorTipWindows');
    if (isMac()) return t('outputSettings.pasteAtCursorTipMac');
    if (isLinux()) return t('outputSettings.pasteAtCursorTipLinux');
    return '';
  }

  function toggleOutputMode(mode: string) {
    let newOutputTo = [...currentOutputTo];

    // GUARD: Don't allow removing the last output - at least one must remain
    if (newOutputTo.includes(mode) && newOutputTo.length === 1) {
      // Cannot remove last output action
      return;
    }

    // GUARD: Don't allow removing clipboard when paste_at_cursor depends on it
    if (mode === 'copy_to_clipboard' && pasteRequiresClipboard) {
      return;
    }

    if (newOutputTo.includes(mode)) {
      newOutputTo = newOutputTo.filter(v => v !== mode);
    } else {
      newOutputTo.push(mode);
    }

    // When enabling paste_at_cursor, ensure clipboard is also enabled
    if (mode === 'paste_at_cursor' && newOutputTo.includes('paste_at_cursor')) {
      if (!newOutputTo.includes('copy_to_clipboard')) {
        newOutputTo.push('copy_to_clipboard');
      }
    }

    // Safety net: ensure at least one output (shouldn't reach here due to guard above)
    if (newOutputTo.length === 0) {
      newOutputTo = ['copy_to_clipboard'];
    }

    updateSetting('output_to', newOutputTo);
  }
</script>

<SettingsSection title={t('settings.output')} icon="output">
  <!-- Format Selector -->
  <div class="setting-row">
    <div class="setting-label-with-tooltip">
      <span class="setting-label">{t('outputSettings.format')}</span>
      {#if $settings?.output_format === 'json'}
        <span class="tooltip-hint">{t('outputSettings.customizeTemplateBelow')}</span>
      {/if}
    </div>
    {#if $settings}
      <div class="select-wrapper">
        <select
          class="select-field"
          value={$settings.output_format || 'text'}
          onchange={e => updateSetting('output_format', e.currentTarget.value)}
        >
          <option value="text">{t('outputSettings.plainText')}</option>
          {#each getAvailableOutputFormats(t) as format}
            {#if format.value !== 'text'}
              <option value={format.value}>{format.label}</option>
            {/if}
          {/each}
        </select>
        <svg class="select-arrow" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"
          ></path>
        </svg>
      </div>
    {/if}
  </div>

  <!-- JSON Metadata Template (shown directly below Format when JSON is selected) -->
  {#if $settings && $settings.output_format === 'json'}
    <JsonTemplateEditor
      template={$settings.json_metadata_template ?? null}
      onUpdate={value => updateSetting('json_metadata_template', value)}
    />
  {/if}

  <!-- Post-Record Actions -->
  <div class="subsection-header">
    <span class="subsection-title">{t('outputSettings.postRecordActions')}</span>
    <span class="subsection-hint">{t('outputSettings.chooseOnePlus')}</span>
  </div>

  {#each getAvailableOutputModes(t) as mode}
    {@const isCopyToClipboard = mode.value === 'copy_to_clipboard'}
    {@const isPasteAtCursor = mode.value === 'paste_at_cursor'}
    {@const isForcedByPaste = isCopyToClipboard && pasteRequiresClipboard}
    {@const isActive = isCopyToClipboard
      ? currentOutputTo.includes('copy_to_clipboard') || !hasUsableOtherOutput || isForcedByPaste
      : currentOutputTo.includes(mode.value)}
    {@const isForcedOn = isCopyToClipboard && (!hasUsableOtherOutput || isForcedByPaste)}
    {@const canToggle = isActive || !isForcedOn}
    {@const isSaveAsFile = mode.value === 'save_as_file'}

    <div class="setting-row" class:locked={isForcedOn}>
      <div class="setting-label-with-tooltip">
        <span class="setting-label">
          {mode.label}
          {#if isCopyToClipboard && isForcedByPaste}
            <span class="hint-trigger" title={t('outputSettings.requiredByPaste')}>
              <svg class="hint-icon" viewBox="0 0 16 16" fill="currentColor">
                <path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z" />
                <path
                  d="M5.255 5.786a.237.237 0 0 0 .241.247h.825c.138 0 .248-.113.266-.25.09-.656.54-1.134 1.342-1.134.686 0 1.314.343 1.314 1.168 0 .635-.374.927-.965 1.371-.673.489-1.206 1.06-1.168 1.987l.003.217a.25.25 0 0 0 .25.246h.811a.25.25 0 0 0 .25-.25v-.105c0-.718.273-.927 1.01-1.486.609-.463 1.244-.977 1.244-2.056 0-1.511-1.276-2.241-2.673-2.241-1.267 0-2.655.59-2.75 2.286zm1.557 5.763c0 .533.425.927 1.01.927.609 0 1.028-.394 1.028-.927 0-.552-.42-.94-1.029-.94-.584 0-1.009.388-1.009.94z"
                />
              </svg>
              <span class="hint-tooltip">{t('outputSettings.requiredByPaste')}</span>
            </span>
          {:else if isCopyToClipboard}
            <span class="default-hint">{t('outputSettings.default')}</span>
          {/if}
          {#if isPasteAtCursor}
            <span class="hint-trigger" title={getPasteAtCursorTip()}>
              <svg class="hint-icon" viewBox="0 0 16 16" fill="currentColor">
                <path d="M8 15A7 7 0 1 1 8 1a7 7 0 0 1 0 14zm0 1A8 8 0 1 0 8 0a8 8 0 0 0 0 16z" />
                <path
                  d="M5.255 5.786a.237.237 0 0 0 .241.247h.825c.138 0 .248-.113.266-.25.09-.656.54-1.134 1.342-1.134.686 0 1.314.343 1.314 1.168 0 .635-.374.927-.965 1.371-.673.489-1.206 1.06-1.168 1.987l.003.217a.25.25 0 0 0 .25.246h.811a.25.25 0 0 0 .25-.25v-.105c0-.718.273-.927 1.01-1.486.609-.463 1.244-.977 1.244-2.056 0-1.511-1.276-2.241-2.673-2.241-1.267 0-2.655.59-2.75 2.286zm1.557 5.763c0 .533.425.927 1.01.927.609 0 1.028-.394 1.028-.927 0-.552-.42-.94-1.029-.94-.584 0-1.009.388-1.009.94z"
                />
              </svg>
              <span class="hint-tooltip">{getPasteAtCursorTip()}</span>
            </span>
          {/if}
        </span>
      </div>
      <div class="setting-controls">
        {#if isSaveAsFile && isActive}
          {#if $settings?.save_directory}
            <!-- Open configured directory -->
            <button
              class="dir-btn"
              title={t('outputModes.openDir', { dir: $settings.save_directory })}
              onclick={() =>
                $settings?.save_directory && cmdOpenDirectory($settings.save_directory)}
            >
              <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" width="14" height="14">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                ></path>
              </svg>
            </button>
            <!-- Change directory -->
            <button
              class="dir-btn"
              title={t('outputSettings.changeSaveDir')}
              onclick={pickSaveDirectory}
              disabled={isPickingDirectory}
            >
              <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" width="14" height="14">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"
                ></path>
              </svg>
            </button>
            <!-- Clear directory (revert to dialog) -->
            <button
              class="dir-btn dir-btn-clear"
              title={t('outputSettings.clearSaveDir')}
              onclick={clearSaveDirectory}
            >
              <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" width="14" height="14">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M6 18L18 6M6 6l12 12"
                ></path>
              </svg>
            </button>
          {:else}
            <!-- Pick a directory for auto-save -->
            <button
              class="dir-btn dir-btn-add"
              title={t('outputSettings.setSaveDir')}
              onclick={pickSaveDirectory}
              disabled={isPickingDirectory}
            >
              <svg fill="none" stroke="currentColor" viewBox="0 0 24 24" width="14" height="14">
                <path
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  stroke-width="2"
                  d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"
                ></path>
              </svg>
            </button>
          {/if}
        {/if}
        <button
          class="toggle-switch"
          class:active={isActive}
          class:disabled={!canToggle}
          onclick={() => canToggle && toggleOutputMode(mode.value)}
          role="switch"
          aria-checked={isActive}
          aria-label={mode.label}
          aria-disabled={!canToggle}
          title={isForcedByPaste
            ? t('outputSettings.requiredByPaste')
            : isForcedOn
              ? t('outputSettings.enableOtherToDisable')
              : ''}
        >
          <span class="toggle-thumb"></span>
        </button>
      </div>
    </div>
    {#if isSaveAsFile && isActive && $settings?.save_directory}
      {@const fileExt = $settings?.output_format === 'json' ? 'json' : 'txt'}
      <div class="save-dir-path" title={$settings.save_directory}>
        {truncatePath($settings.save_directory)}/transcript_{'{timestamp}'}.{fileExt}
        {#if dirSaved}
          <span class="dir-feedback saved">{t('outputModes.saved')}</span>
        {/if}
      </div>
    {/if}
    {#if isSaveAsFile && dirError}
      <div class="dir-feedback error">{dirError}</div>
    {/if}
  {/each}
</SettingsSection>

<style>
  /* `.setting-row` + `.setting-label` (incl. `.locked` modifier)
	   come from app.css (shared) */

  .setting-controls {
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  .dir-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    background: rgba(255, 255, 255, 0.06);
    border: none;
    border-radius: 0.25rem;
    color: #71717a;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .dir-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    color: #a1a1aa;
  }

  .dir-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .dir-btn-add {
    color: #84cc16;
  }

  .dir-btn-add:hover:not(:disabled) {
    color: #a3e635;
    background: rgba(132, 204, 22, 0.1);
  }

  .dir-btn-clear {
    color: #71717a;
  }

  .dir-btn-clear:hover:not(:disabled) {
    color: #ef4444;
    background: rgba(239, 68, 68, 0.1);
  }

  .save-dir-path {
    font-size: 0.625rem;
    color: #52525b;
    padding: 0 0 0.25rem;
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.375rem;
  }

  .dir-feedback.saved {
    font-size: 0.625rem;
    color: #84cc16;
    font-weight: 500;
  }

  .dir-feedback.error {
    font-size: 0.625rem;
    color: #f87171;
    text-align: right;
    padding: 0 0 0.25rem;
  }

  /* `.setting-label` comes from app.css */

  .setting-label-with-tooltip {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .tooltip-hint {
    font-size: 0.625rem;
    color: #84cc16;
    background: rgba(132, 204, 22, 0.1);
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
  }

  .default-hint {
    font-size: 0.625rem;
    color: #71717a;
    background: rgba(255, 255, 255, 0.05);
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    margin-left: 0.25rem;
  }

  .hint-trigger {
    position: relative;
    display: inline-flex;
    align-items: center;
    cursor: help;
    margin-left: 0.25rem;
  }

  .hint-icon {
    width: 0.75rem;
    height: 0.75rem;
    color: #a78bfa;
    transition: color 0.15s ease;
  }

  .hint-trigger:hover .hint-icon {
    color: #c4b5fd;
  }

  .hint-tooltip {
    position: absolute;
    left: 50%;
    top: calc(100% + 6px);
    transform: translateX(-50%);
    padding: 0.5rem 0.75rem;
    background: rgba(39, 39, 42, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.375rem;
    font-size: 0.6875rem;
    font-weight: 400;
    color: #a1a1aa;
    white-space: normal;
    max-width: 280px;
    min-width: 180px;
    text-align: center;
    line-height: 1.5;
    opacity: 0;
    visibility: hidden;
    transition:
      opacity 0.15s ease,
      visibility 0.15s ease;
    pointer-events: none;
    z-index: 100;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
  }

  .hint-tooltip::after {
    content: '';
    position: absolute;
    bottom: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 5px solid transparent;
    border-bottom-color: rgba(39, 39, 42, 0.95);
  }

  .hint-trigger:hover .hint-tooltip {
    opacity: 1;
    visibility: visible;
  }

  .select-wrapper {
    position: relative;
    min-width: 140px;
  }

  .select-field {
    width: 100%;
    padding: 0.375rem 1.75rem 0.375rem 0.625rem;
    background: rgba(255, 255, 255, 0.04);
    border: none;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    color: #d4d4d8;
    cursor: pointer;
    appearance: none;
    transition: background 0.15s ease;
  }

  .select-field:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.06);
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
    width: 1rem;
    height: 1rem;
    color: #71717a;
    pointer-events: none;
  }

  .subsection-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 0 0.5rem;
    margin-top: 0.5rem;
  }

  .subsection-title {
    font-size: 0.6875rem;
    font-weight: 600;
    color: #71717a;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .subsection-hint {
    font-size: 0.625rem;
    font-weight: 500;
    color: #84cc16;
    background: rgba(132, 204, 22, 0.12);
    border: 1px solid rgba(132, 204, 22, 0.25);
    padding: 0.125rem 0.5rem;
    border-radius: 0.25rem;
  }

  /* Toggle Switch */
  .toggle-switch {
    position: relative;
    width: 2.25rem;
    height: 1.25rem;
    background: rgba(255, 255, 255, 0.08);
    border: none;
    border-radius: 0.625rem;
    cursor: pointer;
    transition: background 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    flex-shrink: 0;
  }

  .toggle-switch:hover:not(.disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .toggle-switch:active:not(.disabled) {
    background: rgba(255, 255, 255, 0.08);
  }

  .toggle-switch.active {
    background: rgba(132, 204, 22, 0.35);
  }

  .toggle-switch.active:hover {
    background: rgba(132, 204, 22, 0.45);
  }

  .toggle-switch.disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .toggle-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 0.875rem;
    height: 0.875rem;
    background: #52525b;
    border-radius: 50%;
    transition:
      left 0.2s cubic-bezier(0.34, 1.56, 0.64, 1),
      background 0.2s ease,
      box-shadow 0.2s ease;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
  }

  .toggle-switch.active .toggle-thumb {
    left: calc(100% - 1.125rem);
    background: #84cc16;
    box-shadow: 0 0 8px rgba(132, 204, 22, 0.4);
  }

  /* JSON Syntax Highlighting (still global; rendered via highlightJSON()
	   inside JsonTemplateEditor). */
  :global(.json-key) {
    color: #93c5fd;
  }
  :global(.json-string) {
    color: #86efac;
  }
  :global(.json-number) {
    color: #fcd34d;
  }
  :global(.json-boolean) {
    color: #c4b5fd;
  }
  :global(.json-punctuation) {
    color: #71717a;
  }
  :global(.json-template) {
    color: #f472b6;
    font-weight: 600;
  }
</style>
