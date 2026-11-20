<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * JSON Metadata Template Editor
   *
   * Self-contained editor for the user's `json_metadata_template` setting:
   * Edit / Preview toggle, syntax highlighting, live validation, prettify
   * on blur, and a reset-to-defaults button.
   *
   * Extracted from OutputSection.svelte. Caller owns persistence via the
   * `onUpdate` callback; this component never touches the settings store
   * or IPC directly.
   */

  import { DEFAULT_JSON_TEMPLATE } from '$lib/composables/useTranscriptFormatter';
  import { t } from '$lib/locales';
  import {
    prettifyJSONTemplate,
    validateJSONTemplate,
    generateTemplatePreview,
    highlightJSON,
  } from './outputActions';

  interface Props {
    /** Raw template string from settings (null = use default). */
    template: string | null;
    /** Persist a new value (null clears back to default). */
    onUpdate: (value: string | null) => void | Promise<void>;
  }

  let { template, onUpdate }: Props = $props();

  let editorMode = $state<'edit' | 'preview'>('edit');
  let isValid = $state(true);
  let displayed = $state(DEFAULT_JSON_TEMPLATE);

  // Reflect upstream template changes into the displayed (prettified) value.
  $effect(() => {
    const next = template || DEFAULT_JSON_TEMPLATE;
    displayed = prettifyJSONTemplate(next);
  });

  // Surface validity for the upstream value (parents may want to drive a badge).
  $effect(() => {
    if (template) {
      isValid = validateJSONTemplate(template);
    }
  });
</script>

<div class="metadata-template-section">
  <div class="metadata-template-header">
    <span class="setting-label">{t('outputSettings.metadataTemplate')}</span>
    <div class="template-header-controls">
      <div class="mode-toggle">
        <button
          class="mode-toggle-btn"
          class:active={editorMode === 'edit'}
          onclick={() => (editorMode = 'edit')}
        >
          {t('outputSettings.json')}
        </button>
        <button
          class="mode-toggle-btn"
          class:active={editorMode === 'preview'}
          onclick={() => (editorMode = 'preview')}
        >
          {t('outputSettings.preview')}
        </button>
      </div>
      <button
        class="reset-template-btn"
        onclick={() => {
          onUpdate(null);
          isValid = true;
        }}
        aria-label={t('outputSettings.resetTemplateAria')}
      >
        {t('outputSettings.resetDefaults')}
      </button>
    </div>
  </div>
  <p class="setting-hint" style="margin-bottom: 0.5rem;">
    {t('outputSettings.templateHint')}
  </p>

  {#if editorMode === 'edit'}
    <div class="template-editor-container">
      {#key displayed}
        <div
          class="template-editor editable"
          contenteditable="true"
          oninput={e => {
            const value = (e.target as HTMLElement).innerText;
            isValid = validateJSONTemplate(value);
          }}
          onblur={async e => {
            const value = (e.target as HTMLElement).innerText;
            const finalValue = prettifyJSONTemplate(value);
            await onUpdate(finalValue);
          }}
          role="textbox"
          aria-multiline="true"
          tabindex="0"
        >
          {@html highlightJSON(displayed)}
        </div>
      {/key}
    </div>
  {:else}
    <div class="template-preview">
      {@html highlightJSON(generateTemplatePreview(template || DEFAULT_JSON_TEMPLATE))}
    </div>
  {/if}

  <!-- Validation Status -->
  <div class="template-validation" class:valid={isValid} class:invalid={!isValid}>
    {#if isValid}
      <svg class="validation-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
      </svg>
      <span>{t('outputSettings.validJson')}</span>
    {:else}
      <svg class="validation-icon" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M6 18L18 6M6 6l12 12"
        />
      </svg>
      <span>{t('outputSettings.invalidJson')}</span>
    {/if}
  </div>
</div>

<style>
  .metadata-template-section {
    margin-top: 0.5rem;
  }

  .setting-hint {
    font-size: 0.6875rem;
    color: #52525b;
  }

  .metadata-template-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    margin-bottom: 0.5rem;
  }

  .template-header-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .mode-toggle {
    display: flex;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 0.375rem;
    padding: 0.125rem;
  }

  .mode-toggle-btn {
    padding: 0.25rem 0.5rem;
    font-size: 0.625rem;
    font-weight: 500;
    color: #52525b;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .mode-toggle-btn:hover:not(.active) {
    color: #71717a;
  }

  .mode-toggle-btn.active {
    background: rgba(255, 255, 255, 0.08);
    color: #d4d4d8;
  }

  .reset-template-btn {
    padding: 0.25rem 0.5rem;
    font-size: 0.625rem;
    font-weight: 500;
    color: #52525b;
    background: transparent;
    border: none;
    border-radius: 0.25rem;
    cursor: pointer;
    transition:
      background 0.15s ease,
      color 0.15s ease;
  }

  .reset-template-btn:hover {
    background: rgba(255, 255, 255, 0.04);
    color: #71717a;
  }

  .template-editor-container {
    position: relative;
  }

  .template-editor,
  .template-preview {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 0.6875rem;
    line-height: 1.6;
    padding: 0.75rem;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 0.5rem;
    white-space: pre-wrap;
    word-break: break-word;
    color: #e4e4e7;
  }

  .template-editor.editable {
    outline: none;
    min-height: 100px;
  }

  .template-editor.editable:focus {
    border-color: rgba(132, 204, 22, 0.3);
  }

  .template-validation {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    margin-top: 0.5rem;
    font-size: 0.6875rem;
  }

  .template-validation.valid {
    color: #84cc16;
  }

  .template-validation.invalid {
    color: #ef4444;
  }

  .validation-icon {
    width: 0.875rem;
    height: 0.875rem;
  }
</style>
