<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import DeformedCircleVisualizer from 'visualization/src/lib/visualizations/core/DeformedCircleVisualizer.svelte';
  // @ts-expect-error - visualization package from GitHub has no type declarations
  import Glow from 'visualization/src/lib/visualizations/core/Glow.svelte';

  import type { Snippet } from 'svelte';

  type Props = {
    isActive?: boolean;
    /** Visualizer color (default: teal) */
    color?: string;
    size?: number;
    glow?: number;
    children?: Snippet;
  };

  let { isActive = false, color = '#14b8a6', size = 200, glow = 15, children }: Props = $props();

  let audioContext: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let mediaStream: MediaStream | null = null;
  let animationFrame: number | null = null;
  // Use regular array for Svelte reactivity (typed arrays don't trigger updates reliably)
  let values: number[] = Array(32).fill(0);

  // Smoothed values for nicer animation
  let smoothedValues = new Float32Array(32).fill(0);
  const smoothingFactor = 0.3;

  // Convert to Float32Array for the visualizer component
  let valuesFloat32 = $derived(new Float32Array(values));

  async function startAudioCapture() {
    try {
      // Request microphone access just for visualization
      mediaStream = await navigator.mediaDevices.getUserMedia({
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
        },
      });

      audioContext = new AudioContext();
      analyser = audioContext.createAnalyser();
      analyser.fftSize = 128;
      analyser.smoothingTimeConstant = 0.4;

      const source = audioContext.createMediaStreamSource(mediaStream);
      source.connect(analyser);

      // Start animation loop
      updateValues();
    } catch (error) {
      console.warn('AudioVisualizer: Could not access microphone for visualization:', error);
    }
  }

  function updateValues() {
    if (!analyser || !isActive) {
      // Fade out smoothly when not active
      for (let i = 0; i < smoothedValues.length; i++) {
        smoothedValues[i] *= 0.9;
      }
      values = [...smoothedValues];

      if (smoothedValues.some(v => v > 0.01)) {
        animationFrame = requestAnimationFrame(updateValues);
      }
      return;
    }

    const dataArray = new Uint8Array(analyser.frequencyBinCount);
    analyser.getByteFrequencyData(dataArray);

    // Normalize and smooth values
    const newValues: number[] = [];
    const step = Math.floor(dataArray.length / 32);

    for (let i = 0; i < 32; i++) {
      // Average nearby frequencies for smoother visualization
      let sum = 0;
      for (let j = 0; j < step; j++) {
        sum += dataArray[i * step + j] / 255;
      }
      const rawValue = sum / step;

      // Apply smoothing
      smoothedValues[i] = smoothedValues[i] * (1 - smoothingFactor) + rawValue * smoothingFactor;
      newValues.push(smoothedValues[i]);
    }

    // Assign new array to trigger Svelte reactivity
    values = newValues;
    animationFrame = requestAnimationFrame(updateValues);
  }

  function stopAudioCapture() {
    if (animationFrame) {
      cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }

    if (mediaStream) {
      mediaStream.getTracks().forEach(track => track.stop());
      mediaStream = null;
    }

    if (audioContext) {
      audioContext.close();
      audioContext = null;
    }

    analyser = null;
  }

  // Watch for active state changes
  $effect(() => {
    if (isActive) {
      startAudioCapture();
    } else if (audioContext) {
      // Keep animating to fade out, but stop capturing
      updateValues();
    }
  });

  onMount(() => {
    if (isActive) {
      startAudioCapture();
    }
  });

  onDestroy(() => {
    stopAudioCapture();
  });
</script>

<div class="audio-visualizer-wrapper">
  <!-- Visualization overlay - doesn't affect layout -->
  <div class="audio-visualizer-canvas" style="--size: {size}px;" class:active={isActive}>
    <Glow {glow}>
      <DeformedCircleVisualizer
        values={valuesFloat32}
        {color}
        minRadius={0.35}
        maxRadius={0.5}
        blur={12}
      />
    </Glow>
  </div>

  <!-- Slot content - maintains natural size -->
  {@render children?.()}
</div>

<style>
  .audio-visualizer-wrapper {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .audio-visualizer-canvas {
    position: absolute;
    width: var(--size);
    height: var(--size);
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.3s ease-out;
  }

  .audio-visualizer-canvas.active {
    opacity: 1;
  }

  .audio-visualizer-canvas :global(canvas) {
    width: 100%;
    height: 100%;
  }
</style>
