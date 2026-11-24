<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { t } from '$lib/locales';
  import { useCopyFeedback } from '$lib/composables/useCopyFeedback';
  import { versionReady } from '$lib/composables/usePageLoadAnimation';
  import { useTauriSettings } from '$lib/composables/useTauriSettings';
  import { formatGpuDebug, type GpuInfo, type OsInfo } from '$lib/utils/GpuUtil';
  import { APP_ENV } from '$lib/config/environment';
  import { mustInvoke } from '$lib/ipc/invoke';
  import { onDestroy } from 'svelte';
  import { Icon } from '$lib/icons';
  import { openReleaseNotes } from '$lib/stores/updater';
  import { demoSession } from '$lib/stores/demo';
  // The scripted marketing demo shows the launch version, not the dev build's.
  const DEMO_VERSION = 'v1.0.0';

  /** App version string (e.g., "0.8.0") */
  export let appVersion: string;
  /** GPU info for debug output */
  export let gpuInfo: GpuInfo;
  /** OS info for debug output */
  export let osInfo: OsInfo;
  /** Currently selected model ID */
  export let currentSelectedModel: string | null;

  const copyFeedback = useCopyFeedback();
  // Extract store for $ prefix usage
  const isCopied = copyFeedback.isCopied;
  const { settings } = useTauriSettings();
  let hovered = false;
  let notesHovered = false;
  // Toggle to bring back the inline Release Notes button next to the version label.
  const showReleaseNotesButton = false;

  // Compact display: "v0.8.0-beta.37" -> "beta.37"; otherwise "v<version>".
  $: betaMatch = appVersion?.match(/-beta\.(\d+)/);
  $: displayVersion = $demoSession
    ? DEMO_VERSION
    : betaMatch
      ? `beta.${betaMatch[1]}`
      : `v${appVersion ?? ''}`;

  async function copyDebugInfo() {
    // ort native device enumeration.
    let ortDevices: string;
    try {
      const devices =
        await mustInvoke<Array<{ ep_name: string; hw_vendor: string; device_type: string }>>(
          'list_ort_devices'
        );
      ortDevices =
        devices.length > 0
          ? devices.map(d => `${d.ep_name}/${d.hw_vendor}(${d.device_type})`).join(', ')
          : 'none';
    } catch {
      ortDevices = 'error';
    }

    const debugLines = [
      `Version: \`${appVersion}\``,
      `OS: \`${osInfo.displayString}\``,
      `Env: \`${APP_ENV}\``,
      ...formatGpuDebug(gpuInfo),
      `VoiceToTextModel: \`${currentSelectedModel || 'none'}\``,
      `TransformModel: \`${$settings?.transform_model || 'none'}\``,
      `UsingTransformModel: \`${$settings?.transform_enabled ?? false}\``,
      `OrtDevices: \`${ortDevices}\``,
    ];

    await copyFeedback.copy(debugLines.join('\n'));
  }

  onDestroy(() => copyFeedback.cleanup());
</script>

{#if appVersion}
  <div
    class="fixed bottom-4 right-3 flex items-center gap-1.5 transition-opacity duration-200 ease-out {$versionReady
      ? 'opacity-100'
      : 'opacity-0'}"
  >
    <!-- Release notes trigger hidden for now; re-enable by setting showReleaseNotesButton=true. -->
    {#if showReleaseNotesButton}
      <button
        type="button"
        aria-label={t('update.viewReleaseNotes')}
        title={t('update.viewReleaseNotes')}
        on:click={openReleaseNotes}
        on:mouseenter={() => (notesHovered = true)}
        on:mouseleave={() => (notesHovered = false)}
        class="flex items-center gap-1 text-xs select-none transition-all duration-200 ease-out cursor-pointer outline-none focus:outline-none {notesHovered
          ? 'text-zinc-300 bg-zinc-800/80 px-2 py-0.5 rounded-md'
          : 'text-zinc-500/50 px-1 py-0.5'}"
      >
        <Icon name="release-notes" class="h-3.5 w-3.5" strokeWidth={2} />
        {#if notesHovered}
          <span class="font-medium">{t('update.viewReleaseNotes')}</span>
        {/if}
      </button>
    {/if}
    <button
      on:click={copyDebugInfo}
      on:mouseenter={() => (hovered = true)}
      on:mouseleave={() => (hovered = false)}
      class="text-xs font-mono select-none transition-all duration-200 ease-out cursor-pointer outline-none focus:outline-none {hovered
        ? 'text-zinc-400 bg-zinc-800/80 px-2 py-0.5 rounded-md'
        : 'text-zinc-500/50'}"
    >
      {#if $isCopied}
        <span class="text-lime-400">{t('common.copied')}</span>
      {:else if hovered}
        {t('versionIndicator.copyDebugInfo')}
      {:else}
        {displayVersion}
      {/if}
    </button>
  </div>
{/if}
