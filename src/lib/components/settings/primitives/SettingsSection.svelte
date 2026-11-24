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
    /** Icon name; not rendered by this component (kept so call sites stay unchanged) */
    icon?: string;
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
