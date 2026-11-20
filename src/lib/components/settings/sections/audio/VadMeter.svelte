<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Live audio meter rendered while the VAD test is active.
   *
   * Pure visualization: takes the current level / peak / threshold values
   * and renders the bar, peak marker, and threshold marker. Lifecycle
   * (Tauri listener, timers, IPC) is owned by useVadTest().
   */
  import { t } from '$lib/locales';

  interface Props {
    level: number;
    peak: number;
    peakOpacity: number;
    isSpeech: boolean;
    thresholdValue: number;
  }

  let { level, peak, peakOpacity, isSpeech, thresholdValue }: Props = $props();

  function meterColor(l: number): string {
    if (l < 50) return '#84cc16'; // lime-500
    if (l < 70) return '#eab308'; // yellow-500
    if (l < 85) return '#f97316'; // orange-500
    return '#ef4444'; // red-500
  }
</script>

<div class="audio-meter-container">
  <div class="audio-meter">
    <div class="meter-fill" style="width: {level}%; background: {meterColor(level)}"></div>
    {#if peak > 0}
      <div class="peak-marker" style="left: {peak}%; opacity: {peakOpacity}"></div>
    {/if}
    <div class="threshold-marker" style="left: {thresholdValue}%"></div>
  </div>
  <div class="meter-labels">
    <span class="meter-level">{level}%</span>
    <span class="speech-indicator" class:active={isSpeech}>
      {isSpeech ? t('audioInput.speechDetected') : t('audioInput.listening')}
    </span>
  </div>
</div>

<style>
  .audio-meter-container {
    margin-bottom: 0.75rem;
    padding: 0.625rem;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 0.375rem;
  }

  .audio-meter {
    position: relative;
    height: 8px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    overflow: hidden;
  }

  .meter-fill {
    position: absolute;
    top: 0;
    left: 0;
    height: 100%;
    border-radius: 4px;
    transition:
      width 16ms linear,
      background 50ms ease; /* 60fps smooth, minimal lag */
  }

  .threshold-marker {
    position: absolute;
    top: -2px;
    width: 2px;
    height: 12px;
    background: #06b6d4;
    border-radius: 1px;
    transform: translateX(-50%);
    box-shadow: 0 0 4px rgba(6, 182, 212, 0.5);
  }

  .peak-marker {
    position: absolute;
    top: 0;
    width: 3px;
    height: 100%;
    background: #f472b6;
    border-radius: 1px;
    transform: translateX(-50%);
    box-shadow: 0 0 6px rgba(244, 114, 182, 0.6);
    transition: opacity 0.1s ease;
  }

  .meter-labels {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 0.5rem;
    font-size: 0.625rem;
  }

  .meter-level {
    color: #71717a;
    font-weight: 500;
  }

  .speech-indicator {
    color: #52525b;
    transition: color 0.15s ease;
  }

  .speech-indicator.active {
    color: #84cc16;
  }
</style>
