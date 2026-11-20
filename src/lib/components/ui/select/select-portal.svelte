<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { Select as SelectPrimitive } from 'bits-ui';

  let { ...restProps }: SelectPrimitive.PortalProps = $props();
</script>

<SelectPrimitive.Portal {...restProps} />
