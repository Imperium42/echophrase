<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Feature Badge
   *
   * Small maturity badge shown beside a section title or model name.
   * Variants: 'dev' (blue), 'coming-soon' (gray), 'alpha' (amber - experimental).
   */

  import { t } from '$lib/locales';

  interface Props {
    variant: 'dev' | 'coming-soon' | 'alpha';
    /** Show as section-level tag (larger) vs inline tag */
    section?: boolean;
  }

  let { variant, section = false }: Props = $props();

  const labels = $derived({
    dev: t('badges.development'),
    'coming-soon': t('badges.comingSoon'),
    alpha: t('badges.alpha'),
  });
</script>

<span
  class="feature-badge"
  class:section
  class:dev={variant === 'dev'}
  class:coming-soon={variant === 'coming-soon'}
  class:alpha={variant === 'alpha'}
>
  {labels[variant]}
</span>

<style>
  .feature-badge {
    display: inline-flex;
    align-items: center;
    font-size: 0.5625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    white-space: nowrap;
  }

  /* Section-level badges sit directly beside the section title rather than
	   being flung to the far right edge, where they read as unattached to the
	   heading they qualify. */
  .feature-badge.section {
    margin-left: 0;
  }

  /* Dev variant - subtle blue */
  .feature-badge.dev {
    background: rgba(59, 130, 246, 0.1);
    color: #60a5fa;
  }

  /* Alpha variant - amber maturity flag */
  .feature-badge.alpha {
    background: rgba(245, 158, 11, 0.1);
    color: #fbbf24;
  }

  /* Coming soon variant - subtle gray */
  .feature-badge.coming-soon {
    background: rgba(113, 113, 122, 0.1);
    color: #71717a;
  }
</style>
