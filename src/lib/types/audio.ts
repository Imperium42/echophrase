// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Audio Types and Constants
 * Centralized audio configuration
 */

/** Audio quality settings */
export enum AudioQuality {
  Low = 'low',
  Medium = 'medium',
  High = 'high',
  Ultra = 'ultra',
}

/** Audio format types */
export enum AudioFormat {
  WAV = 'wav',
  MP3 = 'mp3',
  WEBM = 'webm',
  OGG = 'ogg',
}

/** Recording status states */
export enum RecordingStatus {
  Idle = 'idle',
  Initializing = 'initializing',
  Recording = 'recording',
  Paused = 'paused',
  Processing = 'processing',
  Complete = 'complete',
  Error = 'error',
}

/** Audio configuration interface */
export interface AudioConfig {
  sampleRate: number;
  channels: number;
  bitDepth: number;
  format: AudioFormat;
  quality: AudioQuality;
}

/** Recording statistics */
export interface RecordingStats {
  duration: number;
  fileSize: number;
  peakLevel: number;
  averageLevel: number;
}

/** Audio device info */
export interface AudioDevice {
  id: string;
  name: string;
  type: 'input' | 'output';
  isDefault: boolean;
  sampleRates: number[];
}

/** Audio processing options */
export interface AudioProcessingOptions {
  noiseReduction: boolean;
  echoCancellation: boolean;
  autoGainControl: boolean;
  voiceActivityDetection: boolean;
}

/** Audio quality presets */
export const AUDIO_PRESETS: Record<AudioQuality, AudioConfig> = {
  [AudioQuality.Low]: {
    sampleRate: 8000,
    channels: 1,
    bitDepth: 8,
    format: AudioFormat.WAV,
    quality: AudioQuality.Low,
  },
  [AudioQuality.Medium]: {
    sampleRate: 16000,
    channels: 1,
    bitDepth: 16,
    format: AudioFormat.WAV,
    quality: AudioQuality.Medium,
  },
  [AudioQuality.High]: {
    sampleRate: 44100,
    channels: 1,
    bitDepth: 16,
    format: AudioFormat.WAV,
    quality: AudioQuality.High,
  },
  [AudioQuality.Ultra]: {
    sampleRate: 48000,
    channels: 2,
    bitDepth: 24,
    format: AudioFormat.WAV,
    quality: AudioQuality.Ultra,
  },
};

/** Audio level thresholds */
export const AUDIO_LEVELS = {
  SILENCE_THRESHOLD: -50, // dB
  NOISE_FLOOR: -60, // dB
  PEAK_WARNING: -3, // dB
  CLIPPING_THRESHOLD: 0, // dB
} as const;

/**
 * Calculate audio duration from file size and config
 */
export function calculateAudioDuration(fileSize: number, config: AudioConfig): number {
  const bytesPerSample = config.bitDepth / 8;
  const bytesPerSecond = config.sampleRate * config.channels * bytesPerSample;
  return fileSize / bytesPerSecond;
}

/**
 * Convert dB to linear scale
 */
export function dbToLinear(db: number): number {
  return Math.pow(10, db / 20);
}

/**
 * Convert linear scale to dB
 */
export function linearToDb(linear: number): number {
  return 20 * Math.log10(Math.max(0.00001, linear));
}

/**
 * Check if audio level is clipping
 */
export function isClipping(level: number): boolean {
  return level >= AUDIO_LEVELS.CLIPPING_THRESHOLD;
}

/**
 * Check if audio is too quiet
 */
export function isSilent(level: number): boolean {
  return level <= AUDIO_LEVELS.SILENCE_THRESHOLD;
}
