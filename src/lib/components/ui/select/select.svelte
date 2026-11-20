<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { Select as SelectPrimitive } from 'bits-ui';

  let {
    open = $bindable(false),
    value = $bindable(),
    ...restProps
  }: SelectPrimitive.RootProps = $props();
</script>

<SelectPrimitive.Root bind:open bind:value={value as never} {...restProps} />
