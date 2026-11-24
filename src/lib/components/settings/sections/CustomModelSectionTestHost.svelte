<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Test-only harness: sets the real settings context (via the real,
   * unmocked `setSettingsContext`) before mounting `CustomModelSection`, so
   * the component test can exercise `getSettingsContext()` without
   * reimplementing Svelte's context wiring. Not part of the app - only
   * imported from CustomModelSection.test.ts.
   */
  import { setSettingsContext, type SettingsContext } from '../context';
  import CustomModelSection from './CustomModelSection.svelte';

  interface Props {
    context: SettingsContext;
  }

  let { context }: Props = $props();

  // Context is set once at mount, matching how the real SettingsModal wires up
  // its context provider.
  // svelte-ignore state_referenced_locally
  setSettingsContext(context);
</script>

<CustomModelSection />
