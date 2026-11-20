<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Skeleton Loading Placeholder
   *
   * Reusable shimmer primitive for loading states.
   * Renders a rounded rect with pulse animation.
   */

  interface Props {
    /** Width (CSS value) */
    width?: string;
    /** Height (CSS value) */
    height?: string;
    /** Border radius (CSS value) */
    rounded?: string;
    /** Additional CSS class */
    class?: string;
  }

  let {
    width = '100%',
    height = '1rem',
    rounded = '0.375rem',
    class: className = '',
  }: Props = $props();
</script>

<div
  class="skeleton {className}"
  style="width: {width}; height: {height}; border-radius: {rounded};"
></div>

<style>
  .skeleton {
    position: relative;
    background: #27272a;
    overflow: hidden;
  }

  .skeleton::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(90deg, transparent 25%, rgba(63, 63, 70, 0.6) 50%, transparent 75%);
    transform: translateX(-100%);
    animation: skeleton-sweep 1.5s ease-in-out infinite;
  }

  @keyframes skeleton-sweep {
    0% {
      transform: translateX(-100%);
    }
    100% {
      transform: translateX(100%);
    }
  }
</style>
