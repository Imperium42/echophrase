// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Application settings configuration and types
 */

/** Output destination modes for transcription results */
export type OutputMode = 'paste_at_cursor' | 'copy_to_clipboard' | 'save_as_file';

/** Recording mode - push-to-talk (default) or open mic (continuous listening) */
export type RecordingMode = 'push_to_talk' | 'open_mic';

/** UI scale mode - auto (system default size, no extra zoom) or manual slider value */
export type UiScaleMode = 'auto' | 'manual';

/** Chinese script conversion applied to transcription output */
export type ChineseScript = 'traditional' | 'simplified';

/** Main application settings interface - must match Rust AppSettings */
export interface Settings {
  model: string;
  language: string;
  theme: 'light' | 'dark' | 'system';
  output_format: 'text' | 'json'; // TODO: 'vtt' not yet implemented
  hotkey: string;
  /**
   * Cosmetic display names for individual hotkey keys, keyed by the normalized
   * (lowercase) key token - eg `{ f15: 'M5' }` renders the F15 binding as "M5".
   * Never affects the binding itself. See hotkeyFormatter.
   */
  hotkey_aliases?: Record<string, string>;
  auto_copy: boolean; // Deprecated - replaced by output_to
  output_to: OutputMode[]; // Output destinations after transcription (array)
  show_timestamps: boolean;
  compute_type: 'auto' | 'int8' | 'float16' | 'float32';
  custom_model_name: string | null;
  custom_model_url: string | null;
  max_recording_time: number | null; // Max recording time in seconds (5-180)
  show_hotkey_changer: boolean; // Show/hide hotkey customization UI
  show_realtime_preview: boolean; // Show streaming transcription preview during recording
  show_output_confirmations?: boolean; // Show toast confirmations on output (default true)
  show_copy_last_button?: boolean; // Show copy last transcription button (default true)
  show_input_changer?: boolean; // Show input device changer button (default true)
  show_model_switcher?: boolean; // Show model switcher in main UI (default true)
  show_transcription_speed?: boolean; // Show transcription speed toast (default false)
  append_space_incomplete?: boolean; // Append trailing space when output lacks sentence-ending punctuation (default true)
  hotkey_mode?: 'hybrid' | 'toggle' | 'hold'; // Push to Talk hotkey behavior (default hybrid: tap toggles, hold+release stops)
  execution_provider?: string; // Preferred execution provider: directml, coreml, cpu
  // Transform model settings
  transform_enabled?: boolean; // Enable post-processing with LLM
  transform_model?: string | null; // Transform model ID (eg "qwen2.5-0.5b-instruct")
  transform_rules?: string; // Prompt/rules for transformation
  // Save directory for auto-save
  save_directory?: string | null; // Directory for auto-saving transcripts
  // Open mic (continuous listening) settings
  recording_mode?: RecordingMode; // Recording mode: push_to_talk or open_mic
  silence_timeout?: number; // Seconds of silence before auto-transcribing (2-10, default 5)
  // Tray behavior
  minimize_to_tray_on_close?: boolean; // Hide to tray on window X click (default true)
  tray_close_notice_shown?: boolean; // Whether the first-close tray notification was already shown
  // Chinese script conversion for transcription output (models with Eastern language support)
  chinese_script?: ChineseScript; // traditional (default) | simplified
  // Accessibility - UI scaling (applied via CSS zoom on :root)
  ui_scale_mode?: UiScaleMode; // auto = system default size / no extra zoom (default), manual = use ui_scale_manual
  ui_scale_manual?: number; // Manual scale factor, clamped 0.5-2.0 (default 1.0)
  ui_scale_with_window?: boolean; // Additionally scale UI with window size (default false)
  // Optimization: free the model's memory after idle time (opt-in)
  idle_unload_enabled?: boolean; // Unload the speech model when idle (default false)
  idle_unload_minutes?: number; // Idle minutes before unload, 1-120 (default 10)
}

/** Dropdown option item structure */
export interface OptionItem {
  value: string;
  label: string;
}

/** Default transform rules prompt */
export const DEFAULT_TRANSFORM_RULES =
  'Fix grammar, spelling, and punctuation errors in this transcription. Preserve the original meaning and wording. Output only the corrected, normalized text.';

/** Default application settings - must match Rust AppSettings::default() */
export const defaultSettings: Settings = {
  model: 'medium', // Default model (matches Rust)
  language: 'auto',
  theme: 'dark',
  output_format: 'text',
  hotkey: 'ctrl+shift+space', // lowercase hotkeys-js format (matches Rust default + DEFAULT_HOTKEY)
  hotkey_aliases: {}, // No key renamed by default
  auto_copy: true, // Deprecated - kept for backward compatibility
  output_to: ['paste_at_cursor'], // Default to paste at cursor
  show_timestamps: false,
  compute_type: 'auto', // Auto-detect optimal compute type
  custom_model_name: null,
  custom_model_url: null,
  max_recording_time: 30, // Default 30 seconds
  show_hotkey_changer: true, // Show hotkey customization UI by default
  show_realtime_preview: true, // Show realtime transcription preview by default
  show_output_confirmations: true, // Show toast confirmations by default
  show_copy_last_button: true, // Show copy last button by default
  show_input_changer: true, // Show input changer by default
  show_model_switcher: true, // Show model switcher by default
  show_transcription_speed: false, // Don't show speed by default
  append_space_incomplete: true, // Append trailing space for incomplete sentences by default
  hotkey_mode: 'hybrid', // Tap toggles, hold+release stops
  // Transform model defaults
  transform_enabled: false, // Disabled by default
  transform_model: 't5-mini-grammar', // Recommended DLC tier (matches Rust DEFAULT_TRANSFORM_MODEL_ID)
  transform_rules: DEFAULT_TRANSFORM_RULES,
  // Accessibility - UI scaling defaults (match Rust AppSettings::default())
  ui_scale_mode: 'auto',
  ui_scale_manual: 1.0,
  chinese_script: 'traditional',
  ui_scale_with_window: false,
  idle_unload_enabled: false,
  idle_unload_minutes: 10,
};

/** Available language options for transcription */
export const availableLanguages: OptionItem[] = [
  { value: 'auto', label: 'Auto-detect' },
  { value: 'en', label: 'English' },
  { value: 'es', label: 'Spanish' },
  { value: 'fr', label: 'French' },
  { value: 'de', label: 'German' },
  { value: 'it', label: 'Italian' },
  { value: 'pt', label: 'Portuguese' },
  { value: 'ru', label: 'Russian' },
  { value: 'ja', label: 'Japanese' },
  { value: 'ko', label: 'Korean' },
  { value: 'zh', label: 'Chinese' },
];

/** Available Whisper model options */
export const availableModels: OptionItem[] = [
  { value: 'tiny', label: 'Tiny INT8 (39MB) - ~20x realtime' },
  { value: 'base', label: 'Base INT8 (74MB) - ~18x realtime' },
  { value: 'small', label: 'Small INT8 (120MB) - ~15x realtime' },
  { value: 'medium', label: 'Medium INT8 (390MB) - ~12x realtime' },
  { value: 'large-v3', label: 'Large-v3 Turbo INT8 (800MB) - ~8x realtime' },
];

/** Available UI theme options (localized) */
export function getAvailableThemes(t: (key: string) => string): OptionItem[] {
  return [
    { value: 'light', label: t('themes.light') },
    { value: 'dark', label: t('themes.dark') },
    { value: 'system', label: t('themes.system') },
  ];
}

/** Available transcript output format options (localized) */
export function getAvailableOutputFormats(t: (key: string) => string): OptionItem[] {
  return [
    { value: 'text', label: t('outputFormats.plainText') },
    { value: 'json', label: t('outputFormats.jsonWithMetadata') },
  ];
}

/** Available output destination options (localized) */
export function getAvailableOutputModes(
  t: (key: string) => string
): { value: OutputMode; label: string; description: string }[] {
  return [
    {
      value: 'copy_to_clipboard',
      label: t('outputModes.copyToClipboard'),
      description: t('outputModes.copyToClipboardDesc'),
    },
    {
      value: 'paste_at_cursor',
      label: t('outputModes.pasteAtCursor'),
      description: t('outputModes.pasteAtCursorDesc'),
    },
    {
      value: 'save_as_file',
      label: t('outputModes.saveAsFile'),
      description: t('outputModes.saveAsFileDesc'),
    },
  ];
}

/** Available compute type options for CTranslate2 (localized) */
export function getAvailableComputeTypes(t: (key: string) => string): OptionItem[] {
  return [
    { value: 'auto', label: t('computeTypes.autoDetect') },
    { value: 'int8', label: t('computeTypes.int8') },
    { value: 'float16', label: t('computeTypes.fp16') },
    { value: 'float32', label: t('computeTypes.fp32') },
  ];
}
