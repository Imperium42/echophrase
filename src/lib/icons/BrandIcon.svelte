<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Internal renderer for Simple Icons brand glyphs. Use `<Icon name="brand-..." />`
   * from `$lib/icons` instead of importing this directly.
   */
  export let title: string;
  export let path: string;
  export let size: number | string = 16;
  /** CSS color value. Defaults to `currentColor` when not provided. */
  export let color: string = 'currentColor';
  export let ariaLabel: string | undefined = undefined;

  let className = '';
  export { className as class };

  $: dim = typeof size === 'number' ? `${size}px` : size;
  $: labelled = ariaLabel ?? title;
</script>

<svg
  role="img"
  aria-label={labelled}
  width={dim}
  height={dim}
  viewBox="0 0 24 24"
  fill={color}
  class={className}
  xmlns="http://www.w3.org/2000/svg"
>
  <title>{labelled}</title>
  <path d={path} />
</svg>
