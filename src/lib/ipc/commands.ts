// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Typed Tauri command wrappers
 *
 * One exported function per backend command. All IPC call sites should import
 * from here instead of calling invoke() directly, so types are enforced at the
 * boundary and `as any` / `as unknown as` casts are unnecessary.
 *
 * Command names match `#[tauri::command]` fn names in src-tauri/src/commands/.
 */

import { mustInvoke, safeInvoke } from './invoke';
import type { Settings } from '$lib/types/settings';

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

/** Load all application settings from the Rust backend (YAML file). */
export function cmdGetSettings(): Promise<Settings> {
  return mustInvoke<Settings>('get_settings');
}

/** Persist all application settings to the Rust backend (YAML file). */
export function cmdSaveSettings(s: Settings): Promise<void> {
  return mustInvoke<void>('save_settings', { settings: s });
}

// ---------------------------------------------------------------------------
// Model management
// ---------------------------------------------------------------------------

/** Check whether every known model exists on disk. Returns [modelId, exists][]. */
export function cmdCheckModelsExist(): Promise<[string, boolean][]> {
  return mustInvoke<[string, boolean][]>('check_models_exist');
}

/** Check whether the currently-selected model exists on disk. */
export function cmdCheckSelectedModelExists(): Promise<boolean> {
  return mustInvoke<boolean>('check_selected_model_exists');
}

/** Check whether the ONNX Runtime dylib is available. */
export function cmdCheckOrtAvailable(): Promise<boolean> {
  return mustInvoke<boolean>('check_ort_available');
}

/** Download (or resume) the currently-selected Whisper model. */
export function cmdDownloadWhisperModel(): Promise<void> {
  return mustInvoke<void>('download_whisper_model');
}

/** Cancel an in-progress model download. */
export function cmdCancelModelDownload(): Promise<void> {
  return mustInvoke<void>('cancel_model_download');
}

/** Delete a downloaded model from disk. */
export function cmdDeleteModel(modelType: string): Promise<void> {
  return mustInvoke<void>('delete_model', { modelType });
}

/** Preload the selected Whisper model into GPU memory. */
export function cmdPreloadWhisperModel(): Promise<void> {
  return mustInvoke<void>('preload_whisper_model');
}

/** Unload all models from memory (transcription + transform). */
export function cmdUnloadModel(): Promise<void> {
  return mustInvoke<void>('unload_model');
}

/** Switch the active model download target. */
export function cmdSwitchModelDownload(modelType: string): Promise<void> {
  return mustInvoke<void>('switch_model_download', { modelType });
}

// ---------------------------------------------------------------------------
// Transform model
// ---------------------------------------------------------------------------

/** Unload the transform (grammar/correction) model from memory. */
export function cmdUnloadTransformModel(): Promise<void> {
  return mustInvoke<void>('unload_transform_model');
}

// ---------------------------------------------------------------------------
// Directory / file system
// ---------------------------------------------------------------------------

/** Path to the directory containing downloaded voice models. */
export function cmdGetModelsDir(): Promise<string> {
  return mustInvoke<string>('get_models_dir');
}

/** Open a directory in the OS file manager. */
export function cmdOpenDirectory(path: string): Promise<void> {
  return mustInvoke<void>('open_directory', { path });
}

// ---------------------------------------------------------------------------
// Debug / dev tools
// ---------------------------------------------------------------------------

/** Toggle the Tauri webview developer tools panel. */
export function cmdToggleDevtools(): Promise<void> {
  return mustInvoke<void>('toggle_devtools');
}

/** Retrieve recent debug log lines from the Rust logger. */
export function cmdGetDebugLogs(): Promise<string[]> {
  return mustInvoke<string[]>('get_debug_logs');
}

// ---------------------------------------------------------------------------
// Model auto-detection
// ---------------------------------------------------------------------------

/** Run hardware detection and return the recommended model. */
export interface AutoDetectionResult {
  recommended_model: string;
  gpu_name: string | null;
  vram_mb: number | null;
  reason: string;
}

export function cmdGetModelRecommendation(): Promise<AutoDetectionResult> {
  return mustInvoke<AutoDetectionResult>('get_model_recommendation');
}

export function cmdMaybeAutoDetectModel(): Promise<AutoDetectionResult | null> {
  return mustInvoke<AutoDetectionResult | null>('maybe_auto_detect_model');
}

export function cmdForceAutoDetectModel(): Promise<AutoDetectionResult> {
  return mustInvoke<AutoDetectionResult>('force_auto_detect_model');
}

export function cmdResetModelDetection(): Promise<void> {
  return mustInvoke<void>('reset_model_detection');
}

// ---------------------------------------------------------------------------
// GPU / execution provider
// ---------------------------------------------------------------------------

export interface ExecutionProviderSelection {
  provider: string;
  reason: string;
}

export interface DeviceInfo {
  name: string;
  device_type: string;
  memory_mb: number | null;
}

export function cmdGetExecutionProvider(): Promise<ExecutionProviderSelection> {
  return mustInvoke<ExecutionProviderSelection>('get_execution_provider');
}

export function cmdListOrtDevices(): Promise<DeviceInfo[]> {
  return mustInvoke<DeviceInfo[]>('list_ort_devices');
}

export function cmdSetPreferredExecutionProvider(provider: string): Promise<void> {
  return mustInvoke<void>('set_preferred_execution_provider', { provider });
}

// ---------------------------------------------------------------------------
// System info
// ---------------------------------------------------------------------------

export interface SystemInfo {
  os: string;
  arch: string;
  cpu: string;
  ram_mb: number;
}

export function cmdGetSystemInfo(): Promise<SystemInfo> {
  return mustInvoke<SystemInfo>('get_system_info');
}

export function cmdIsWsl(): Promise<boolean> {
  return mustInvoke<boolean>('is_wsl');
}
