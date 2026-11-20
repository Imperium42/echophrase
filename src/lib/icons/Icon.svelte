<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * The single icon component for the entire app. Pass a semantic `name`
   * registered in `registry.ts` — never import from `@lucide/svelte` or
   * `simple-icons` directly.
   *
   * @example
   *   <Icon name="warning" size={16} class="text-amber-400" />
   *   <Icon name="brand-discord" size={20} ariaLabel="Discord" />
   *   <Icon name="brand-windows" size={24} />  // brand color (#0078D4)
   *   <Icon name="brand-windows" size={24} color="currentColor" />  // override
   */
  import { ICONS, type IconName } from './registry';
  import BrandIcon from './BrandIcon.svelte';

  export let name: IconName;
  export let size: number | string = 16;
  /** Lucide stroke width; ignored for brand icons. */
  export let strokeWidth: number | string = 2;
  /**
   * CSS color override. For Lucide icons, defaults to `currentColor`.
   * For brand icons, defaults to the brand's official hex (or `currentColor`
   * when `monochrome: true` in the registry, e.g. Apple, X).
   */
  export let color: string | undefined = undefined;
  /**
   * Accessible label. Required for icon-only interactive controls. Decorative
   * icons (next to text labels) can omit this — the host element should carry
   * the label and the icon is implicitly hidden.
   */
  export let ariaLabel: string | undefined = undefined;

  let className = '';
  export { className as class };

  $: def = ICONS[name];
  $: brandColor =
    def.kind === 'brand'
      ? (color ?? (def.monochrome ? 'currentColor' : `#${def.hex}`))
      : 'currentColor';
</script>

{#if def.kind === 'lucide'}
  <svelte:component
    this={def.component}
    {size}
    {strokeWidth}
    color={color ?? 'currentColor'}
    aria-label={ariaLabel}
    aria-hidden={ariaLabel ? undefined : 'true'}
    class={className}
  />
{:else}
  <BrandIcon
    title={def.title}
    path={def.path}
    {size}
    color={brandColor}
    {ariaLabel}
    class={className}
  />
{/if}
