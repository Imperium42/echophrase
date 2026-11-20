<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { t } from '$lib/locales';
  import { Icon } from '$lib/icons';

  type Props = {
    /** Friendly device label (default: system default) */
    deviceName?: string;
    /** Actual OS-level device name from system */
    actualInputName?: string;
  };

  // Future enhancement: Could detect actual microphone device name via Web API or Tauri command
  let { deviceName = 'System Default Microphone', actualInputName = '' }: Props = $props();

  // Format display name for system default
  const displayName = $derived(
    deviceName === 'System Default Microphone' && actualInputName
      ? `${t('audioInput.systemDefault')} (${actualInputName.length > 12 ? actualInputName.slice(0, 12) + '..' : actualInputName})`
      : deviceName === 'System Default Microphone'
        ? t('mic.systemDefault')
        : deviceName
  );
</script>

<!-- Microphone Device Status -->
<div class="text-center">
  <div class="flex items-center justify-center gap-2 text-zinc-400 text-sm">
    <Icon name="mic" class="w-4 h-4" />
    <span>{displayName}</span>
  </div>
</div>
