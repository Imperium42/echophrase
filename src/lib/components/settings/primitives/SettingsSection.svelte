<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Settings Section
   *
   * Flat, modern section wrapper with subtle separator.
   * No card styling - clean, minimal design.
   */

  import type { Snippet } from 'svelte';
  import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
  } from '$lib/components/ui/collapsible';
  import FeatureBadge from './FeatureBadge.svelte';
  import { Icon } from '$lib/icons';
  import { t } from '$lib/locales';

  interface Props {
    /** Section ID for scroll-to-section */
    id?: string;
    /** Section title */
    title: string;
    /** SVG icon path (d attribute) or preset name */
    icon: string;
    /** Make section collapsible (default: true) */
    collapsible?: boolean;
    /** Default open state (for collapsible) */
    defaultOpen?: boolean;
    /** Maturity badge shown beside the title (e.g., 'coming-soon') */
    featureTag?: 'dev' | 'coming-soon' | 'alpha' | null;
    /** Extra badge shown alongside featureTag (e.g. 'alpha' for an experimental feature) */
    extraBadge?: 'alpha' | null;
    /** Extra content to render after title (eg help icon) */
    titleExtra?: Snippet;
    /** Section content */
    children: Snippet;
  }

  let {
    id,
    title,
    icon,
    collapsible = true,
    defaultOpen = true,
    featureTag = null,
    extraBadge = null,
    titleExtra,
    children,
  }: Props = $props();

  // Intentionally capture initial value only - isOpen is then managed independently by user interaction
  // svelte-ignore state_referenced_locally
  let isOpen = $state(defaultOpen);

  // Icon paths for common section icons
  const iconPaths: Record<string, string> = {
    model:
      'M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z',
    gpu: 'M9 3v2m6-2v2M9 19v2m6-2v2M5 9H3m2 6H3m18-6h-2m2 6h-2M7 19h10a2 2 0 002-2V7a2 2 0 00-2-2H7a2 2 0 00-2 2v10a2 2 0 002 2zM9 9h6v6H9V9z',
    optimization: 'M13 10V3L4 14h7v7l9-11h-7z',
    output:
      'M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z',
    recording:
      'M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z',
    'audio-input':
      'M12 18.75a6 6 0 006-6v-1.5m-6 7.5a6 6 0 01-6-6v-1.5m6 7.5v3.75m-3.75 0h7.5M12 15.75a3 3 0 01-3-3V4.5a3 3 0 116 0v8.25a3 3 0 01-3 3z',
    hotkeys:
      'M15 15l-2 5L9 9l11 4-5 2zm0 0l5 5M7.188 2.239l.777 2.897M5.136 7.965l-2.898-.777M13.95 4.05l-2.122 2.122m-5.657 5.656l-2.12 2.122',
    customize:
      'M11 4a2 2 0 114 0v1a1 1 0 001 1h3a1 1 0 011 1v3a1 1 0 01-1 1h-1a2 2 0 100 4h1a1 1 0 011 1v3a1 1 0 01-1 1h-3a1 1 0 01-1-1v-1a2 2 0 10-4 0v1a1 1 0 01-1 1H7a1 1 0 01-1-1v-3a1 1 0 00-1-1H4a2 2 0 110-4h1a1 1 0 001-1V7a1 1 0 011-1h3a1 1 0 001-1V4z',
    accessibility:
      'M12 5a1.5 1.5 0 100-3 1.5 1.5 0 000 3zM4 7.5l6.5 1v4l-2.4 7.2a1 1 0 001.9.7L12 14l1.999 6.4a1 1 0 001.9-.7l-2.4-7.2v-4l6.5-1a1 1 0 10-.3-2L14 6.5h-4L4.3 5.5a1 1 0 10-.3 2z',
    'custom-model':
      'M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4',
    data: 'M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z',
    debug: 'M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z',
    transform:
      'M7 8h10M7 12h4m1 8l-4-4H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-3l-4 4z',
    mcp: 'M8 9l3 3-3 3m5 0h3M4 5h16a1 1 0 011 1v12a1 1 0 01-1 1H4a1 1 0 01-1-1V6a1 1 0 011-1z',
  };

  const iconPath = $derived(iconPaths[icon] || icon);
</script>

{#if collapsible}
  <Collapsible bind:open={isOpen}>
    <section {id} class="settings-section">
      <CollapsibleTrigger
        class="section-header clickable"
        title={isOpen ? t('settings.collapseSection') : t('settings.expandSection')}
        aria-label={isOpen ? t('settings.collapseSection') : t('settings.expandSection')}
      >
        <span class="section-title">{title}</span>
        {#if featureTag}
          <FeatureBadge variant={featureTag} section />
        {/if}
        {#if extraBadge}
          <FeatureBadge variant={extraBadge} section />
        {/if}
        <Icon
          name="chevron-down"
          class="chevron {isOpen ? 'open' : 'collapsed'}"
          size={12}
          strokeWidth={2}
        />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div class="section-content">
          {@render children()}
        </div>
      </CollapsibleContent>
    </section>
  </Collapsible>
{:else}
  <section {id} class="settings-section">
    <div class="section-header">
      <span class="section-title">{title}</span>
      {#if titleExtra}
        {@render titleExtra()}
      {/if}
      {#if featureTag}
        <FeatureBadge variant={featureTag} section />
      {/if}
    </div>
    <div class="section-content">
      {@render children()}
    </div>
  </section>
{/if}

<style>
  .settings-section {
    background: transparent;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
    padding: 1rem 0;
    margin: 0;
    transition: background 0.3s ease;
    cursor: default;
    user-select: none;
  }

  .settings-section:last-child {
    border-bottom: none;
  }

  /* Highlight effect for scroll-to-section */
  .settings-section:global(.section-highlight) {
    background: rgba(132, 204, 22, 0.03);
  }

  .section-header,
  .settings-section :global(.section-header) {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0;
    margin-bottom: 0.75rem;
    font-size: 0.625rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.3);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    width: 100%;
    background: none;
    border: none;
    cursor: default;
    transition: color 0.15s ease;
  }

  .settings-section :global(.section-header.clickable) {
    cursor: pointer !important;
    user-select: none;
    border-radius: 0.25rem;
    /* Only inset the hover/focus box on the left + vertical edges so the
		   chevron's right edge stays flush with the section's right inset -
		   matching the right-aligned icons (eg the docs "book" icon) used
		   inside section content. Do NOT add right padding/margin here. */
    margin: -0.25rem 0 -0.25rem -0.25rem;
    padding: 0.25rem 0 0.25rem 0.25rem;
  }

  .settings-section :global(.section-header.clickable:hover) {
    color: rgba(255, 255, 255, 0.45);
    background: rgba(255, 255, 255, 0.02);
  }

  .settings-section :global(.section-header.clickable:active) {
    background: rgba(255, 255, 255, 0.01);
  }

  .section-title {
    text-align: left;
  }

  /* The title no longer stretches (so gate badges sit beside it), so the
	   chevron takes over pushing itself to the right edge. */
  :global(.chevron) {
    margin-left: auto;
    color: rgba(255, 255, 255, 0.2);
    transition:
      transform 0.2s cubic-bezier(0.4, 0, 0.2, 1),
      opacity 0.15s ease;
    flex-shrink: 0;
  }

  /* Expanded: chevron only hints on hover ("click to collapse"). */
  :global(.chevron.open) {
    transform: rotate(180deg);
    opacity: 0;
  }

  .settings-section :global(.section-header.clickable:hover .chevron.open),
  .settings-section :global(.section-header.clickable:focus-visible .chevron.open) {
    opacity: 1;
  }

  /* Collapsed: chevron is always visible so it's obvious the section can reopen. */
  :global(.chevron.collapsed) {
    opacity: 1;
  }

  .section-content {
    padding: 0;
  }
</style>
