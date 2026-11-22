// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable, get } from 'svelte/store';
import { updateSettings } from '$lib/ipc/settings';
import { mustInvoke } from '$lib/ipc/invoke';
import { defaultSettings, type Settings } from '$lib/types/settings';

export interface TauriSettings {
  model: string;
  /** Spoken-language hint ("auto" or ISO code). Consumed by backends that support it (Qwen3-ASR). */
  language: string;
  output_format: string;
  hotkey: string;
  auto_copy: boolean; // Deprecated - replaced by output_to
  output_to: string[]; // Array of: paste_at_cursor, copy_to_clipboard, save_as_file
  /** Reserved: no UI or output path reads this yet. Kept for settings-file compatibility. */
  show_timestamps: boolean;
  compute_type: string;
  custom_model_name?: string | null;
  custom_model_url?: string | null;
  max_recording_time?: number;
  show_hotkey_changer: boolean; // Show/hide hotkey customization UI
  show_realtime_preview: boolean; // Show streaming transcription preview during recording
  show_output_confirmations: boolean; // Show toast confirmations after output actions
  show_copy_last_button: boolean; // Show button to copy last transcription to clipboard
  show_input_changer?: boolean; // Show input device changer button
  show_model_switcher?: boolean; // Show model switcher in main UI
  show_transcription_speed?: boolean; // Show transcription speed toast
  show_last_on_hover?: boolean; // Show last transcription on hover near record button
  remove_filler_words: boolean; // Remove um, uh, etc. from transcription output
  use_numeric_digits: boolean; // Display numbers as digits (1, 2) instead of text (one, two)
  append_space_incomplete?: boolean; // Append trailing space when output lacks sentence-ending punctuation (default true)
  hotkey_mode?: 'hybrid' | 'toggle' | 'hold'; // Push to Talk hotkey behavior (default hybrid: tap toggles, hold+release stops)
  restore_clipboard_after_paste?: boolean; // Put the previous clipboard text back after paste at cursor (default false)
  json_metadata_template?: string | null; // Custom JSON template for metadata output
  telemetry_enabled: boolean; // Anonymous usage stats and crash reports (opt-out)
  // Audio input settings (available to all users)
  input_device?: string; // Audio input device ID (default = system default)
  voice_threshold?: number; // 0-100, VAD threshold for voice detection
  mcp_server_enabled?: boolean; // Localhost IPC server for CLI/MCP transcribe
  // Transform model settings
  transform_enabled?: boolean; // Enable post-processing with LLM
  transform_model?: string | null; // Transform model ID
  transform_rules?: string; // Prompt/rules for transformation
  // Save directory for auto-save
  save_directory?: string | null; // Directory for auto-saving transcripts
  // Open mic (continuous listening) settings
  recording_mode?: 'push_to_talk' | 'open_mic'; // Recording mode (default: push_to_talk)
  silence_timeout?: number; // Seconds of silence before auto-transcribing (2-10, default 5)
  // Tray behavior
  minimize_to_tray_on_close?: boolean; // Hide to tray on window X click (default true)
  tray_close_notice_shown?: boolean; // Whether the first-close tray notification was already shown
  // Chinese script conversion for transcription output (Eastern-language models)
  chinese_script?: 'traditional' | 'simplified';
  // Accessibility - UI scaling (applied via CSS zoom on :root by useUiScale)
  ui_scale_mode?: 'auto' | 'manual'; // auto = system default size / no extra zoom (default)
  ui_scale_manual?: number; // Manual scale factor, clamped 0.5-2.0 (default 1.0)
  ui_scale_with_window?: boolean; // Additionally scale UI with window size (default false)
  // Optimization: free the model's memory after idle time (opt-in)
  idle_unload_enabled?: boolean; // default false
  idle_unload_minutes?: number; // 1-120, default 10
}

// Single source of truth for the default; mirrors the Rust default in
// settings/mod.rs (`max_recording_time: Some(30)`).
const DEFAULT_MAX_RECORDING_TIME = defaultSettings.max_recording_time ?? 30;

// ============================================================================
// Singleton stores - shared across all components
// ============================================================================
const settingsStore = writable<TauriSettings | null>(null);
const isLoadingStore = writable(true);
const errorStore = writable<string | null>(null);

// Track initialization state
let isInitialized = false;
let invoke: any = null;

/**
 * Initialize Tauri API - called once
 */
async function initializeTauri(): Promise<boolean> {
  if (invoke) return true;

  if (typeof window !== 'undefined' && typeof (window as any).__TAURI_INTERNALS__ !== 'undefined') {
    try {
      const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
      invoke = tauriInvoke;
      return true;
    } catch (error) {
      console.log('Failed to load Tauri APIs:', error);
      return false;
    }
  }
  return false;
}

/**
 * Load settings from Tauri backend - can be called multiple times safely
 */
async function loadSettings(): Promise<void> {
  if (!invoke) {
    const initialized = await initializeTauri();
    if (!initialized) {
      errorStore.set('Tauri APIs not available');
      isLoadingStore.set(false);
      return;
    }
  }

  try {
    isLoadingStore.set(true);
    errorStore.set(null);

    const settingsData = await invoke('get_settings');

    settingsStore.set(settingsData);
    isInitialized = true;
  } catch (err) {
    console.error('Failed to load settings:', err);
    errorStore.set(err instanceof Error ? err.message : 'Failed to load settings');
  } finally {
    isLoadingStore.set(false);
  }
}

/**
 * Save settings to Tauri backend
 */
async function saveSettings(newSettings: TauriSettings): Promise<void> {
  if (!invoke) {
    throw new Error('Tauri APIs not available');
  }

  // Sanitize settings - ensure numeric fields have values (not null)
  const sanitized: TauriSettings = {
    ...newSettings,
    max_recording_time: newSettings.max_recording_time ?? DEFAULT_MAX_RECORDING_TIME,
  };

  try {
    await invoke('save_settings', { settings: sanitized });
    settingsStore.set(sanitized);
  } catch (err) {
    console.error('Failed to save settings:', err);
    throw err;
  }
}

// Debounced persist for `updateSetting` so rapid toggles (slider drags,
// mass-toggle clicks) coalesce into a single disk write per ~150ms window.
// The optimistic store update happens immediately so UI stays responsive.
const SAVE_DEBOUNCE_MS = 150;
let saveDebounceTimer: ReturnType<typeof setTimeout> | null = null;
/**
 * Only the keys changed since the last flush. We send these as a patch rather
 * than a whole-object save: a snapshot save carries every OTHER field too, so
 * it reverted values written through a different path between the snapshot and
 * the flush (that is what made selecting Qwen silently fall back to Parakeet).
 */
let pendingPatch: Partial<TauriSettings> = {};

function flushPendingSave(): void {
  if (saveDebounceTimer !== null) {
    clearTimeout(saveDebounceTimer);
    saveDebounceTimer = null;
  }
  const patch = pendingPatch;
  pendingPatch = {};
  if (Object.keys(patch).length === 0) return;

  void updateSettings(patch as Partial<Settings>)
    .then(updated => {
      // Trust the backend's post-patch state over our optimistic copy.
      settingsStore.set(updated as unknown as TauriSettings);
    })
    .catch(err => console.error('Debounced settings patch failed:', err));
}

/**
 * Update a single setting. Updates the in-memory store immediately and
 * debounces the disk write. Direct callers that need durability before
 * resolving (e.g. import flows) should call `saveSettings` instead.
 */
async function updateSetting<K extends keyof TauriSettings>(
  key: K,
  value: TauriSettings[K]
): Promise<void> {
  const currentSettings = get(settingsStore);
  if (!currentSettings) {
    throw new Error('Settings not loaded');
  }

  const updatedSettings = { ...currentSettings, [key]: value };
  // Optimistic UI update so consumers see the new value before the IPC
  // round-trip completes.
  settingsStore.set(updatedSettings);
  pendingPatch[key] = value;

  if (saveDebounceTimer !== null) {
    clearTimeout(saveDebounceTimer);
  }
  saveDebounceTimer = setTimeout(flushPendingSave, SAVE_DEBOUNCE_MS);
}

/**
 * Toggle the localhost MCP/CLI IPC server via the dedicated Tauri command
 * (not `updateSetting`/`update_settings_patch`): starting or stopping the
 * actual listener needs to happen synchronously with the persisted setting.
 * Throws on error (e.g. port already in use by something else) so the caller can revert its optimistic toggle.
 */
export async function setMcpServerEnabled(enabled: boolean): Promise<void> {
  await mustInvoke<void>('set_mcp_server_enabled', { enabled });
}

/**
 * Refresh the store from the backend, which is authoritative.
 *
 * Called on the Rust `settings-changed` event so a write from anywhere (a
 * store, another command, auto-detection) is reflected in the UI instead of
 * the UI showing a value the backend is not actually using.
 */
export async function refreshSettingsFromBackend(): Promise<void> {
  try {
    const fresh = (await invoke('get_settings')) as TauriSettings;
    settingsStore.set(fresh);
  } catch (err) {
    console.warn('Failed to refresh settings from backend:', err);
  }
}

// Best-effort flush on page hide/unload so the last toggle isn't lost if the
// user closes the window during the debounce window. The addEventListener
// check keeps web-mode/test environments (bare window stubs) working.
if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
  window.addEventListener('beforeunload', flushPendingSave);
  window.addEventListener('pagehide', flushPendingSave);
}

/**
 * Reset settings to defaults
 */
async function resetToDefaults(): Promise<void> {
  const defaultSettings: TauriSettings = {
    language: 'en',
    model: 'small',
    output_format: 'text',
    auto_copy: true, // Deprecated
    output_to: ['paste_at_cursor'], // Default: paste at cursor
    show_timestamps: false,
    hotkey: 'Ctrl+Shift+Space',
    compute_type: 'auto',
    max_recording_time: DEFAULT_MAX_RECORDING_TIME,
    show_hotkey_changer: true,
    show_realtime_preview: true,
    show_output_confirmations: true,
    show_copy_last_button: true,
    show_input_changer: true,
    show_model_switcher: true,
    show_transcription_speed: false,
    remove_filler_words: true,
    use_numeric_digits: false, // Convert "one, two" to "1, 2"
    append_space_incomplete: true, // Append trailing space for incomplete sentences by default
    hotkey_mode: 'hybrid', // Tap toggles, hold+release stops
    restore_clipboard_after_paste: false, // Keep the transcript on the clipboard
    json_metadata_template: null,
    telemetry_enabled: true, // Default ON - anonymous usage stats
    input_device: 'default', // System default audio input
    voice_threshold: 50, // 50% VAD threshold
  };
  await saveSettings(defaultSettings);
}

// ============================================================================
// Exported singleton stores - import these directly in components
// ============================================================================
export const tauriSettings = {
  subscribe: settingsStore.subscribe,
};

export const tauriSettingsLoading = {
  subscribe: isLoadingStore.subscribe,
};

export const tauriSettingsError = {
  subscribe: errorStore.subscribe,
};

// ============================================================================
// Composable for components that need full settings management
// ============================================================================

/**
 * Composable for managing settings through Tauri backend
 * Connects frontend to YAML-based settings storage
 *
 * Uses singleton stores - all components share the same state
 */
export function useTauriSettings() {
  return {
    // State (singleton stores)
    settings: tauriSettings,
    isLoading: tauriSettingsLoading,
    error: tauriSettingsError,

    // Actions
    loadSettings,
    saveSettings,
    updateSetting,
    resetToDefaults,
    initializeTauri,
  };
}
