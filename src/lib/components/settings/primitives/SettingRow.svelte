<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Setting Row
   *
   * Minimal row component for settings items.
   * Clean layout with generous spacing.
   * Hint shown as (?) icon with hover tooltip.
   */

  import type { Snippet } from 'svelte';
  import HintTooltip from './HintTooltip.svelte';
  import WarningTooltip from './WarningTooltip.svelte';
  import { Icon } from '$lib/icons';
  import { launchUrl } from '$lib/utils/launchUrl';
  import { t } from '$lib/locales';

  interface Props {
    /** Main label text */
    label: string;
    /** Optional hint/description shown on (?) hover */
    hint?: string;
    /** Optional warning shown on amber ⚠ hover (flags a gotcha) */
    warning?: string;
    /** Is this setting locked (grayed out) */
    locked?: boolean;
    /** Show "Default" badge next to label */
    isDefault?: boolean;
    /** Optional docs page for this setting, opened by a 📖 button next to the label */
    docsUrl?: string;
    /** Tooltip / a11y label for the docs button (required when docsUrl is set) */
    docsTooltip?: string;
    /** Optional inline content rendered right after the label (eg a value input) */
    afterLabel?: Snippet;
    /** Control component (toggle, select, etc.) */
    children: Snippet;
  }

  let {
    label,
    hint,
    warning,
    locked = false,
    isDefault = false,
    docsUrl,
    docsTooltip,
    afterLabel,
    children,
  }: Props = $props();
</script>

<div class="setting-row" class:locked>
  <div class="setting-info">
    <span class="setting-label">
      {label}
      {#if warning}
        <WarningTooltip {warning} />
      {/if}
      {#if hint}
        <HintTooltip {hint} />
      {/if}
      {#if docsUrl}
        <!-- launchUrl, not a bare anchor: target=_blank does nothing inside the Tauri webview -->
        <button
          class="docs-link"
          type="button"
          title={docsTooltip}
          aria-label={docsTooltip}
          onclick={() => launchUrl(docsUrl)}
        >
          <Icon name="book" size={12} />
        </button>
      {/if}
      {@render afterLabel?.()}
      {#if isDefault}
        <span class="default-hint">{t('outputSettings.default')}</span>
      {/if}
    </span>
  </div>

  <div class="setting-control">
    {@render children()}
  </div>
</div>

<style>
  /* `.setting-row` and `.setting-label` come from app.css (shared with
	   hand-rolled rows in OutputSection / AudioInputSection / etc.). The
	   primitive's variant adds a richer label layout (icon row + tooltips). */

  .setting-info {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    flex: 1;
    min-width: 0;
  }

  /* Override the global label rule with a flex layout for hint/warning icons. */
  .setting-label {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    font-weight: 400;
    transition: color 0.15s ease;
  }

  .setting-row:hover .setting-label {
    color: #e4e4e7;
  }

  .docs-link {
    display: inline-flex;
    align-items: center;
    padding: 0;
    background: none;
    border: none;
    color: #3f3f46;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  .docs-link:hover {
    color: #a78bfa;
  }

  .default-hint {
    font-size: 0.5625rem;
    color: #3f3f46;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .setting-control {
    flex-shrink: 0;
  }
</style>
