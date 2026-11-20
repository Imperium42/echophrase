<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { Collapsible as CollapsiblePrimitive } from 'bits-ui';

  let {
    ref = $bindable(null),
    open = $bindable(false),
    ...restProps
  }: CollapsiblePrimitive.RootProps = $props();
</script>

<CollapsiblePrimitive.Root bind:ref bind:open data-slot="collapsible" {...restProps} />
