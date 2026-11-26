<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * MCP Server Section
   *
   * Toggles the localhost-only IPC server (127.0.0.1:1424) that the CLI's
   * `echophrase-cli transcribe` command and the MCP `transcribe` tool use to
   * drive this already-running app's transcription pipeline, instead of
   * loading a second copy of the model.
   */

  import { SettingsSection, SettingToggle } from '../primitives';
  import { Icon } from '$lib/icons';
  import { getSettingsContext } from '../context';
  import { setMcpServerEnabled } from '$lib/composables/useTauriSettings';
  import { useCopyFeedback } from '$lib/composables/useCopyFeedback';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { docsUrl } from '$lib/utils/docsUrl';
  import { t } from '$lib/locales';
  import { MCP_SETUP_PROMPT } from './mcpSetupPrompt';

  const { settings } = getSettingsContext();

  const enabled = $derived($settings?.mcp_server_enabled ?? false);

  const promptCopy = useCopyFeedback();
  const isPromptCopied = promptCopy.isCopied;

  // Separate timed flag (rather than reusing isPromptCopied) so the side
  // hint's fade timing can be tuned independently of the button label swap.
  let showPasteHint = $state(false);
  let pasteHintTimeout: ReturnType<typeof setTimeout> | null = null;

  function copySetupPrompt(): void {
    void promptCopy.copy(MCP_SETUP_PROMPT);
    showPasteHint = true;
    if (pasteHintTimeout) clearTimeout(pasteHintTimeout);
    pasteHintTimeout = setTimeout(() => {
      showPasteHint = false;
    }, 2500);
  }

  let error = $state<string | null>(null);
  let errorTimeout: ReturnType<typeof setTimeout> | null = null;

  function flashError(message: string): void {
    error = message;
    if (errorTimeout) clearTimeout(errorTimeout);
    errorTimeout = setTimeout(() => {
      error = null;
    }, 4000);
  }

  /**
   * The dedicated command is the single writer for this setting: it
   * persists, starts/stops the listener,
   * and emits `settings-changed` so the store (and this toggle) resync
   * from the backend. No optimistic patch write - a second writer through
   * `updateSetting` raced this command and could persist a stale revert
   * while the listener stayed up.
   */
  async function toggle(value: boolean): Promise<void> {
    error = null;
    try {
      await setMcpServerEnabled(value);
    } catch (err) {
      flashError(err instanceof Error ? err.message : t('mcpServer.errorGeneric'));
    }
  }
</script>

<SettingsSection id="mcp-server" title={t('mcpServer.title')} icon="mcp">
  <SettingToggle
    label={t('mcpServer.label')}
    hint={t('mcpServer.hint')}
    checked={enabled}
    onchange={toggle}
  />

  <div class="copy-prompt-row">
    <button class="copy-prompt-btn" onclick={copySetupPrompt}>
      <Icon name={$isPromptCopied ? 'check' : 'copy'} size={14} />
      {$isPromptCopied ? t('mcpServer.copyPromptCopied') : t('mcpServer.copyPrompt')}
    </button>
    {#if showPasteHint}
      <span class="paste-hint">{t('mcpServer.copyPromptHint')}</span>
    {/if}
  </div>

  <p class="note">
    {t('mcpServer.noteCliPre')}<button
      type="button"
      class="doc-link"
      title={t('mcpServer.openDoc')}
      onclick={() => launchUrl(docsUrl('/cli'))}>{t('mcpServer.noteCli')}</button
    >{t('mcpServer.noteMid')}<button
      type="button"
      class="doc-link"
      title={t('mcpServer.openDoc')}
      onclick={() => launchUrl(docsUrl('/mcp'))}>{t('mcpServer.noteMcp')}</button
    >{t('mcpServer.notePost')}
  </p>

  {#if error}
    <p class="error">{error}</p>
  {/if}
</SettingsSection>

<style>
  .copy-prompt-row {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    margin-top: 0.5rem;
  }

  .paste-hint {
    font-size: 0.75rem;
    font-weight: 500;
    color: #a3e635; /* Tailwind lime-400: readable accent on zinc-950 */
    animation: paste-hint-fade 2.5s ease forwards;
  }

  @keyframes paste-hint-fade {
    0% {
      opacity: 0;
    }
    15% {
      opacity: 1;
    }
    75% {
      opacity: 1;
    }
    100% {
      opacity: 0;
    }
  }

  .doc-link {
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    font: inherit;
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }

  .doc-link:hover {
    color: #e4e4e7;
  }

  .copy-prompt-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    background: rgba(63, 63, 70, 0.5);
    border: 1px solid rgba(82, 82, 91, 0.5);
    border-radius: 0.375rem;
    padding: 0.375rem 0.75rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: #e4e4e7;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .copy-prompt-btn:hover {
    background: rgba(82, 82, 91, 0.6);
    border-color: rgba(113, 113, 122, 0.5);
  }

  .copy-prompt-btn:active {
    transform: scale(0.98);
  }

  .note {
    margin: 0.5rem 0 0;
    font-size: 0.75rem;
    color: #a1a1aa;
    line-height: 1.5;
  }

  .error {
    margin: 0.5rem 0 0;
    padding: 0.5rem 0.75rem;
    border-radius: 0.375rem;
    font-size: 0.8125rem;
    background: rgba(239, 68, 68, 0.12);
    color: #fca5a5;
  }
</style>
