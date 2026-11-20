<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import ModalWindow from './ModalWindow.svelte';
  import { ArrowUpCircle } from '@lucide/svelte';
  import { t } from '$lib/locales';

  interface Props {
    isOpen?: boolean;
    version?: string;
    releaseNotes?: string;
    onUpdate: () => void;
    onLater: () => void;
  }

  let { isOpen = false, version = '', releaseNotes = '', onUpdate, onLater }: Props = $props();

  /** Minimal markdown -> HTML for release notes (headers, bullets, bold, code) */
  function renderMarkdown(md: string): string {
    return (
      md
        // Escape HTML first
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        // Headers
        .replace(/^### (.+)$/gm, '<h3>$1</h3>')
        .replace(/^## (.+)$/gm, '<h2>$1</h2>')
        .replace(/^# (.+)$/gm, '<h1>$1</h1>')
        // Bold
        .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
        // Inline code
        .replace(/`(.+?)`/g, '<code>$1</code>')
        // Bullet lists
        .replace(/^[-*] (.+)$/gm, '<li>$1</li>')
        // Wrap consecutive <li> in <ul>
        .replace(/(<li>[\s\S]*?<\/li>)(\n(?!<li>)|$)/g, (m, list) => `<ul>${list}</ul>`)
        // Line breaks (non-block elements)
        .replace(/\n(?!<[hul])/g, '<br>')
    );
  }
</script>

<ModalWindow {isOpen} onClose={onLater} maxWidth="max-w-lg">
  <div class="update-modal">
    <div class="update-header">
      <ArrowUpCircle class="update-icon" strokeWidth={2} />
      <div>
        <h2 class="update-title">{t('update.updateAvailable')}</h2>
        <p class="update-version">{t('update.versionLabel', { version })}</p>
      </div>
    </div>

    {#if releaseNotes}
      <div class="release-notes">
        <!-- eslint-disable-next-line svelte/no-at-html-tags -->
        {@html renderMarkdown(releaseNotes)}
      </div>
    {/if}

    <div class="update-actions">
      <button class="later-btn" onclick={onLater}>{t('update.later')}</button>
      <button class="update-btn" onclick={onUpdate}>
        <ArrowUpCircle class="w-4 h-4" strokeWidth={2.5} />
        {t('update.updateNow')}
      </button>
    </div>
  </div>
</ModalWindow>

<style>
  .update-modal {
    padding: 0.25rem 0 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .update-header {
    display: flex;
    align-items: center;
    gap: 0.875rem;
  }

  :global(.update-icon) {
    width: 2rem;
    height: 2rem;
    color: #38bdf8;
    flex-shrink: 0;
  }

  .update-title {
    font-size: 1.125rem;
    font-weight: 600;
    color: #e4e4e7;
    margin: 0;
  }

  .update-version {
    font-size: 0.8125rem;
    color: #38bdf8;
    margin: 0.125rem 0 0;
  }

  .release-notes {
    background: rgba(24, 24, 27, 0.6);
    border: 1px solid rgba(63, 63, 70, 0.5);
    border-radius: 0.5rem;
    padding: 0.875rem 1rem;
    max-height: 16rem;
    overflow-y: auto;
    font-size: 0.8125rem;
    color: #a1a1aa;
    line-height: 1.6;
  }

  .release-notes :global(h1),
  .release-notes :global(h2),
  .release-notes :global(h3) {
    color: #d4d4d8;
    font-weight: 600;
    margin: 0.75rem 0 0.25rem;
  }

  .release-notes :global(h1) {
    font-size: 1rem;
  }
  .release-notes :global(h2) {
    font-size: 0.9375rem;
  }
  .release-notes :global(h3) {
    font-size: 0.875rem;
  }

  .release-notes :global(h1:first-child),
  .release-notes :global(h2:first-child),
  .release-notes :global(h3:first-child) {
    margin-top: 0;
  }

  .release-notes :global(ul) {
    margin: 0.25rem 0;
    /* Tailwind's preflight resets ul to list-style: none, so the marker has to
	     be restored explicitly - padding-left alone just indents unmarked text. */
    list-style: disc outside;
    padding-left: 1.25rem;
  }

  .release-notes :global(li) {
    margin: 0.125rem 0;
  }

  .release-notes :global(strong) {
    color: #d4d4d8;
    font-weight: 600;
  }

  .release-notes :global(code) {
    background: rgba(63, 63, 70, 0.6);
    border-radius: 0.25rem;
    padding: 0.1em 0.35em;
    font-family: monospace;
    font-size: 0.875em;
    color: #7dd3fc;
  }

  .update-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 0.25rem;
  }

  .later-btn {
    background: rgba(63, 63, 70, 0.5);
    border: 1px solid rgba(82, 82, 91, 0.5);
    border-radius: 0.375rem;
    padding: 0.5rem 1rem;
    font-size: 0.8125rem;
    color: #a1a1aa;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .later-btn:hover {
    background: rgba(82, 82, 91, 0.6);
  }

  .update-btn {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    background: linear-gradient(135deg, #0ea5e9, #2563eb);
    border: none;
    border-radius: 0.375rem;
    padding: 0.5rem 1rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: white;
    cursor: pointer;
    transition: opacity 0.15s ease;
  }

  .update-btn:hover {
    opacity: 0.9;
  }
</style>
