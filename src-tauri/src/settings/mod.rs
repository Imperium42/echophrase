// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

pub mod cache;
use crate::error::ToCmdErr;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

/// Process-wide cached settings snapshot. Populated on first `load()` and
/// invalidated by `save()`. Hot-path callers (transcription, recording, VAD
/// loops) used to hit the disk on every IPC call - during open mic that
/// fired per VAD chunk. Reads now go through this cache.
/// Env var that, when set, relocates `settings.yaml` into the given directory.
/// Used by tests so they never read or write the real user settings file.
pub const CONFIG_DIR_ENV: &str = "ECHOPHRASE_CONFIG_DIR";

static SETTINGS_CACHE: RwLock<Option<AppSettings>> = RwLock::new(None);

/// Default model ID - matches frontend MODEL_REGISTRY default
pub const DEFAULT_MODEL_ID: &str = "parakeet-tdt-0.6b-v3";

/// Default transform model ID - matches frontend MODEL_REGISTRY recommended.
pub const DEFAULT_TRANSFORM_MODEL_ID: &str = "t5-mini-grammar";

/// Default transform rules prompt
pub const DEFAULT_TRANSFORM_RULES: &str = "Fix grammar, spelling, and punctuation errors in this transcription. Preserve the original meaning and wording. Output only the corrected, normalized text.";

fn default_true() -> bool {
    true
}

fn default_hotkey_mode() -> String {
    "hybrid".to_string()
}

/// Map any hotkey mode string onto the supported set (`toggle`, `hold`,
/// `hybrid`); case-insensitive, unknown values fall back to `hybrid`.
fn normalize_hotkey_mode(mode: &str) -> String {
    match mode.trim().to_lowercase().as_str() {
        "toggle" => "toggle".to_string(),
        "hold" => "hold".to_string(),
        _ => default_hotkey_mode(),
    }
}

fn default_transform_rules() -> String {
    DEFAULT_TRANSFORM_RULES.to_string()
}

/// Default UI scale mode - "auto" follows the OS display scale factor.
fn default_ui_scale_mode() -> String {
    "auto".to_string()
}

fn default_idle_unload_minutes() -> u32 {
    crate::transcription::idle::DEFAULT_IDLE_UNLOAD_MINUTES
}

fn clamp_idle_unload_minutes(minutes: u32) -> u32 {
    use crate::transcription::idle::{MAX_IDLE_UNLOAD_MINUTES, MIN_IDLE_UNLOAD_MINUTES};
    minutes.clamp(MIN_IDLE_UNLOAD_MINUTES, MAX_IDLE_UNLOAD_MINUTES)
}

/// Most transcripts the in-memory history can remember. Keep in step with
/// `HISTORY_LIMIT` in `src/lib/stores/lastRun.ts`.
pub const MAX_HISTORY_LIMIT: u32 = 10;
const MIN_HISTORY_LIMIT: u32 = 1;

/// Where the recording overlay sits on screen. Kept in sync with
/// `OVERLAY_POSITIONS` in `src/lib/types/settings.ts`.
pub const OVERLAY_POSITIONS: [&str; 6] = [
    "top-left",
    "top-center",
    "top-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
];

fn default_overlay_position() -> String {
    "bottom-center".to_string()
}

/// Unknown values fall back to the default instead of failing the patch.
fn normalize_overlay_position(position: &str) -> String {
    if OVERLAY_POSITIONS.contains(&position) {
        position.to_string()
    } else {
        default_overlay_position()
    }
}

fn default_history_limit() -> u32 {
    MAX_HISTORY_LIMIT
}

fn clamp_history_limit(limit: u32) -> u32 {
    limit.clamp(MIN_HISTORY_LIMIT, MAX_HISTORY_LIMIT)
}

/// Default manual UI scale factor (100%).
fn default_ui_scale_manual() -> f64 {
    1.0
}

/// Longest accepted hotkey alias. Aliases render inline in compact UI (the hint
/// under the record button, the settings pill), so an unbounded string would
/// blow out those layouts.
pub const HOTKEY_ALIAS_MAX_LEN: usize = 12;

/// Normalize a hotkey alias map on the way in: keys lowercased to match the
/// canonical token form used by the hotkey string, values trimmed and length
/// capped. Entries whose alias is empty after trimming are dropped, so clearing
/// the text field is how a user removes an alias.
fn normalize_hotkey_aliases(aliases: HashMap<String, String>) -> HashMap<String, String> {
    aliases
        .into_iter()
        .filter_map(|(key, alias)| {
            let alias = alias.trim();
            if alias.is_empty() {
                return None;
            }
            let alias: String = alias.chars().take(HOTKEY_ALIAS_MAX_LEN).collect();
            Some((key.trim().to_lowercase(), alias))
        })
        .collect()
}

/// Maximum voice (VAD) threshold percentage. Values above are clamped on save.
pub const VOICE_THRESHOLD_MAX: u8 = 100;

/// Default VAD threshold (50%), matching the frontend's historical default.
fn default_voice_threshold() -> u8 {
    50
}

/// Allowed UI scale range (50%-200%). Values outside are clamped on save.
pub const UI_SCALE_MIN: f64 = 0.5;
/// See [`UI_SCALE_MIN`].
pub const UI_SCALE_MAX: f64 = 2.0;

/// Custom deserializer for model field that handles migration from old enum values to new string IDs
fn deserialize_model<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;

    // Migrate old enum values to new model IDs
    let model_id = match value.to_lowercase().as_str() {
        // Old enum values / removed Whisper IDs → upgrade to current default
        "medium" | "large" | "whisper-medium" | "whisper-large-v3-turbo" | "distil-large-v3" => {
            DEFAULT_MODEL_ID.to_string()
        }
        // New model IDs pass through as-is
        id if id.starts_with("distil-")
            || id.starts_with("whisper-")
            || id.starts_with("parakeet-")
            // Without this arm every load rewrote a persisted "qwen3-asr-0.6b"
            // back to DEFAULT_MODEL_ID, so auto-detect's Qwen3 pick silently
            // reverted to Parakeet on the next launch.
            || id.starts_with("qwen3-")
            || id == "custom" =>
        {
            value
        }
        // Unknown values default to recommended model
        _ => DEFAULT_MODEL_ID.to_string(),
    };

    Ok(model_id)
}

/// Custom deserializer for `transform_model` that maps any id that is not a
/// Transformation model in the registry to the bundled default. Covers
/// removed tiers (eg the short-lived NC-licensed `t5-base-grammar` /
/// `coedit-large-grammar`) and mis-migrated values that hold a TRANSCRIPTION
/// id (seen in the wild: `parakeet-tdt-0.6b-v3`), which made every grammar
/// correction fail with "missing required encoder/decoder/tokenizer files".
fn deserialize_transform_model<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(value.map(sanitize_transform_model_id))
}

/// Keep `id` only if it names a Transformation model; otherwise the default.
fn sanitize_transform_model_id(id: String) -> String {
    let is_transform = crate::models::get_model_info(&id)
        .is_some_and(|m| m.category == crate::models::ModelCategory::Transformation);
    if is_transform {
        id
    } else {
        DEFAULT_TRANSFORM_MODEL_ID.to_string()
    }
}

/// Custom deserializer for output_to that handles migration from old string format to new array format
fn deserialize_output_to<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};

    struct OutputToVisitor;

    impl<'de> Visitor<'de> for OutputToVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        // Handle old format: output_to: "copy_to_clipboard"
        fn visit_str<E>(self, value: &str) -> std::result::Result<Vec<String>, E>
        where
            E: de::Error,
        {
            if value.is_empty() || value == "none" {
                Ok(vec!["paste_at_cursor".to_string()])
            } else {
                Ok(vec![value.to_string()])
            }
        }

        // Handle new format: output_to: ["copy_to_clipboard", "paste_at_cursor"]
        fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Vec<String>, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element::<String>()? {
                values.push(value);
            }
            if values.is_empty() {
                Ok(vec!["paste_at_cursor".to_string()])
            } else {
                Ok(values)
            }
        }
    }

    deserializer.deserialize_any(OutputToVisitor)
}

/// Deserialize a nullable patch field into `Option<Option<T>>`.
///
/// Distinguishes three states the frontend can express:
/// - key absent      -> `None`          (leave the field alone)
/// - key set to null -> `Some(None)`    (explicitly clear it)
/// - key set to v    -> `Some(Some(v))` (set it)
///
/// Without this, clearing a nullable field (eg `save_directory`) would be
/// indistinguishable from not mentioning it.
fn double_option<'de, T, D>(deserializer: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

/// Partial update to `AppSettings` - every field optional.
///
/// Sent by the frontend so a single-setting change carries ONLY that setting,
/// instead of round-tripping the whole struct (which is how concurrent writers
/// used to clobber each other's fields). Applied inside one
/// `AppSettings::update` closure so the whole patch lands atomically.
///
/// `deny_unknown_fields` is deliberate: without it serde silently DROPS any key it
/// does not recognise, so a frontend setting missing its Rust field looks like it
/// saved and then reverts on the next load. `voice_threshold` shipped that way and
/// could never persist. Failing loudly turns that class of bug into an error at the
/// call site instead of a mystery. Frontend and backend ship in the same binary, so
/// there is no version skew to tolerate here.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct SettingsPatch {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub output_format: Option<String>,
    #[serde(default)]
    pub hotkey: Option<String>,
    #[serde(default)]
    pub auto_copy: Option<bool>,
    #[serde(default)]
    pub output_to: Option<Vec<String>>,
    #[serde(default)]
    pub show_timestamps: Option<bool>,
    #[serde(default)]
    pub compute_type: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub custom_model_name: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub custom_model_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub custom_model_path: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub custom_model_format: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub max_recording_time: Option<Option<u32>>,
    #[serde(default)]
    pub show_hotkey_changer: Option<bool>,
    #[serde(default)]
    pub show_realtime_preview: Option<bool>,
    #[serde(default)]
    pub show_output_confirmations: Option<bool>,
    #[serde(default)]
    pub show_copy_last_button: Option<bool>,
    #[serde(default)]
    pub show_input_changer: Option<bool>,
    #[serde(default)]
    pub show_model_switcher: Option<bool>,
    #[serde(default)]
    pub show_transcription_speed: Option<bool>,
    #[serde(default)]
    pub show_last_on_hover: Option<bool>,
    #[serde(default)]
    pub remove_filler_words: Option<bool>,
    #[serde(default)]
    pub use_numeric_digits: Option<bool>,
    #[serde(default)]
    pub append_space_incomplete: Option<bool>,
    #[serde(default)]
    pub convert_spoken_punctuation: Option<bool>,
    #[serde(default)]
    pub hotkey_mode: Option<String>,
    #[serde(default)]
    pub restore_clipboard_after_paste: Option<bool>,
    #[serde(default, deserialize_with = "double_option")]
    pub json_metadata_template: Option<Option<String>>,
    #[serde(default)]
    pub auto_detected_model: Option<bool>,
    #[serde(default, deserialize_with = "double_option")]
    pub execution_provider: Option<Option<String>>,
    #[serde(default)]
    pub transform_enabled: Option<bool>,
    #[serde(default, deserialize_with = "double_option")]
    pub transform_model: Option<Option<String>>,
    #[serde(default)]
    pub transform_rules: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub save_directory: Option<Option<String>>,
    #[serde(default)]
    pub ui_locale: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub recording_mode: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub silence_timeout: Option<Option<u32>>,
    #[serde(default)]
    pub minimize_to_tray_on_close: Option<bool>,
    #[serde(default)]
    pub tray_close_notice_shown: Option<bool>,
    #[serde(default, deserialize_with = "double_option")]
    pub input_device: Option<Option<String>>,
    #[serde(default)]
    pub chinese_script: Option<String>,
    #[serde(default)]
    pub ui_scale_mode: Option<String>,
    #[serde(default)]
    pub ui_scale_manual: Option<f64>,
    #[serde(default)]
    pub ui_scale_with_window: Option<bool>,
    #[serde(default)]
    pub idle_unload_enabled: Option<bool>,
    #[serde(default)]
    pub idle_unload_minutes: Option<u32>,
    #[serde(default)]
    pub history_enabled: Option<bool>,
    #[serde(default)]
    pub history_limit: Option<u32>,
    #[serde(default)]
    pub recording_overlay_enabled: Option<bool>,
    #[serde(default)]
    pub recording_overlay_position: Option<String>,
    #[serde(default)]
    pub voice_threshold: Option<u8>,
    /// Legacy keys from the removed cloud-sync and hosted-transcription features.
    /// Accepted and ignored so an old exported settings file still imports despite
    /// `deny_unknown_fields`.
    #[serde(default)]
    #[allow(dead_code)]
    pub hosted_api_key: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub cloud_sync_enabled: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub pending_cloud_model: Option<serde_json::Value>,
    #[serde(default)]
    #[allow(dead_code)]
    pub pending_cloud_transform_model: Option<serde_json::Value>,
    #[serde(default)]
    pub mcp_server_enabled: Option<bool>,
    #[serde(default)]
    pub hotkey_aliases: Option<HashMap<String, String>>,
}

impl SettingsPatch {
    /// Apply every present field to `settings`, normalizing the same way
    /// `save_settings` does so both write paths agree.
    pub(crate) fn apply_to(self, s: &mut AppSettings) {
        macro_rules! set {
            ($($field:ident),* $(,)?) => {
                $(if let Some(v) = self.$field { s.$field = v; })*
            };
        }

        // Plain fields.
        set!(
            model,
            language,
            theme,
            output_format,
            auto_copy,
            output_to,
            show_timestamps,
            compute_type,
            show_hotkey_changer,
            show_realtime_preview,
            show_output_confirmations,
            show_copy_last_button,
            show_input_changer,
            show_model_switcher,
            show_transcription_speed,
            show_last_on_hover,
            remove_filler_words,
            use_numeric_digits,
            append_space_incomplete,
            convert_spoken_punctuation,
            restore_clipboard_after_paste,
            auto_detected_model,
            transform_enabled,
            transform_rules,
            ui_locale,
            minimize_to_tray_on_close,
            tray_close_notice_shown,
            chinese_script,
            ui_scale_with_window,
            idle_unload_enabled,
            history_enabled,
            recording_overlay_enabled,
            mcp_server_enabled,
        );

        // Nullable fields: outer Some means "mentioned", inner value is the
        // new state (including an explicit null).
        set!(
            custom_model_name,
            custom_model_url,
            custom_model_path,
            custom_model_format,
            max_recording_time,
            json_metadata_template,
            execution_provider,
            save_directory,
            recording_mode,
            silence_timeout,
            input_device,
        );

        // Normalizations mirroring save_settings so the two paths cannot drift.
        if let Some(hotkey) = self.hotkey {
            s.hotkey = hotkey.to_lowercase();
        }
        if let Some(mode) = self.hotkey_mode {
            s.hotkey_mode = normalize_hotkey_mode(&mode);
        }
        if let Some(model) = self.transform_model {
            s.transform_model = model.map(sanitize_transform_model_id);
        }
        if let Some(aliases) = self.hotkey_aliases {
            s.hotkey_aliases = normalize_hotkey_aliases(aliases);
        }
        if let Some(threshold) = self.voice_threshold {
            s.voice_threshold = threshold.min(VOICE_THRESHOLD_MAX);
        }
        if let Some(minutes) = self.idle_unload_minutes {
            s.idle_unload_minutes = clamp_idle_unload_minutes(minutes);
        }
        if let Some(position) = self.recording_overlay_position {
            s.recording_overlay_position = normalize_overlay_position(&position);
        }
        if let Some(limit) = self.history_limit {
            s.history_limit = clamp_history_limit(limit);
        }
        if let Some(scale) = self.ui_scale_manual {
            s.ui_scale_manual = scale.clamp(UI_SCALE_MIN, UI_SCALE_MAX);
        }
        if let Some(mode) = self.ui_scale_mode {
            s.ui_scale_mode = if mode == "manual" {
                "manual".to_string()
            } else {
                "auto".to_string()
            };
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// Model ID string (e.g., "distil-large-v3", "parakeet-tdt-0.6b-v2")
    /// Matches frontend MODEL_REGISTRY in types/models.ts
    #[serde(deserialize_with = "deserialize_model")]
    pub model: String,
    /// Transcription language hint: "auto" (model-side detection, default) or an
    /// ISO 639-1 / BCP-47 code (eg "en", "zh-TW") forced into multilingual models.
    #[serde(default = "default_language")]
    pub language: String,
    pub theme: String,
    pub output_format: String,
    pub hotkey: String,
    /// User-supplied display names for individual hotkey keys, keyed by the
    /// normalized (lowercase) key token. Purely cosmetic - the binding itself is
    /// unchanged. Lets a user label an unlabelled key on their keyboard, eg
    /// `{"f15": "M5"}` renders the F15 binding as "M5" everywhere in the UI.
    #[serde(default)]
    pub hotkey_aliases: HashMap<String, String>,
    pub auto_copy: bool, // Deprecated - replaced by output_to
    #[serde(deserialize_with = "deserialize_output_to")]
    pub output_to: Vec<String>, // Array of: paste_at_cursor, copy_to_clipboard, save_as_file
    pub show_timestamps: bool,
    pub compute_type: String, // auto, int8, float16, float32
    pub custom_model_name: Option<String>,
    pub custom_model_url: Option<String>,
    /// Directory of the imported BYO custom model (contains `model.onnx` +
    /// `meta.json`). Set only by the import commands; `None` means nothing
    /// imported (selecting "custom" without this errors with a re-import hint).
    #[serde(default)]
    pub custom_model_path: Option<String>,
    /// Format of the imported custom model ("onnx" in v1).
    #[serde(default)]
    pub custom_model_format: Option<String>,
    pub max_recording_time: Option<u32>, // Max recording time in seconds (5-180)
    pub show_hotkey_changer: bool,       // Show/hide hotkey customization UI
    pub show_realtime_preview: bool,     // Show streaming transcription preview during recording
    pub show_output_confirmations: bool, // Show toast confirmations after output actions
    pub show_copy_last_button: bool,     // Show button to copy last transcription to clipboard
    pub show_input_changer: bool,        // Show input device changer button
    pub show_model_switcher: bool,       // Show model switcher in main UI
    pub show_transcription_speed: bool,  // Show transcription speed toast
    #[serde(default = "default_true")]
    pub show_last_on_hover: bool, // Show last transcription on hover near record button
    pub remove_filler_words: bool,       // Remove um, uh, etc. from transcription output
    pub use_numeric_digits: bool, // Display numbers as digits (1, 2) instead of text (one, two)
    #[serde(default = "default_true")]
    pub append_space_incomplete: bool, // Append trailing space when output lacks sentence-ending punctuation
    #[serde(default = "default_true")]
    pub convert_spoken_punctuation: bool, // Turn spoken "question mark" / "exclamation mark" into ? / !
    /// Push to Talk hotkey behavior: "hybrid" (tap toggles, hold+release stops),
    /// "toggle", or "hold". Unknown values behave as hybrid on the frontend.
    #[serde(default = "default_hotkey_mode")]
    pub hotkey_mode: String,
    /// Put the previous clipboard text back after paste at cursor (opt-in;
    /// reading the clipboard may need extra permission on macOS).
    #[serde(default)]
    pub restore_clipboard_after_paste: bool,
    pub json_metadata_template: Option<String>, // Custom JSON template for metadata output
    /// Whether the current model was auto-detected on first launch.
    /// If true, subsequent launches won't re-detect unless user resets.
    /// If false (user manually selected), auto-detection is skipped.
    #[serde(default)]
    pub auto_detected_model: bool,
    /// Preferred execution provider: "directml", "coreml", "cpu", or None (auto-detect)
    #[serde(default)]
    pub execution_provider: Option<String>,
    /// Enable post-processing with transform LLM
    #[serde(default)]
    pub transform_enabled: bool,
    /// Transform model ID (registry id from `MODEL_REGISTRY`, category=Transformation)
    #[serde(default, deserialize_with = "deserialize_transform_model")]
    pub transform_model: Option<String>,
    /// Prompt/rules for transformation
    #[serde(default = "default_transform_rules")]
    pub transform_rules: String,
    /// Directory for auto-saving transcripts (when save_as_file is enabled)
    #[serde(default)]
    pub save_directory: Option<String>,
    /// UI locale for internationalization (e.g., "en", "zh-TW")
    /// Separate from `language` which controls transcription language
    #[serde(default = "default_ui_locale")]
    pub ui_locale: String,
    /// Recording mode: "push_to_talk" (default) or "open_mic" (continuous listening)
    // TODO: Default to "open_mic" on Linux since global hotkey macros aren't reliable there.
    // Also needs a home screen indicator (with good UX) when open mic is active.
    #[serde(default)]
    pub recording_mode: Option<String>,
    /// Seconds of silence before auto-transcribing in open mic mode (2-10, default 5)
    #[serde(default)]
    pub silence_timeout: Option<u32>,
    /// Hide the window to the system tray when the user clicks X, rather than quitting.
    /// Default true on Windows/macOS. On Linux the close handler always quits regardless
    /// (WSL and most tray-less Linux setups have no reliable tray).
    #[serde(default = "default_true")]
    pub minimize_to_tray_on_close: bool,
    /// Whether the first-close tray notification has already been shown.
    /// Once shown once, subsequent hides are silent.
    #[serde(default)]
    pub tray_close_notice_shown: bool,
    /// Selected audio input device id, or `"default"`/`None` for the system
    /// default. On WSL this is the PulseAudio source name (e.g. `RDPSource`);
    /// on Windows/macOS it is the cpal device display name.
    #[serde(default)]
    pub input_device: Option<String>,
    /// Chinese script conversion for transcription output: "traditional"
    /// (Simplified -> Traditional, default) or "simplified" (reverse).
    /// Only surfaced in the UI when the active model supports Eastern languages.
    #[serde(default = "default_chinese_script")]
    pub chinese_script: String,
    /// UI scale mode: "auto" (system default size / no extra zoom) or "manual"
    /// (use `ui_scale_manual`). Applied by the frontend via CSS zoom.
    #[serde(default = "default_ui_scale_mode")]
    pub ui_scale_mode: String,
    /// Manual UI scale factor, clamped to [0.5, 2.0] on save (50%-200%).
    /// Only used when `ui_scale_mode` is "manual".
    #[serde(default = "default_ui_scale_manual")]
    pub ui_scale_manual: f64,
    /// When true, the effective UI scale additionally tracks the window
    /// width relative to a reference width captured at toggle-on (frontend
    /// keeps the reference in memory - never persisted).
    #[serde(default)]
    pub ui_scale_with_window: bool,
    /// Opt-in: free the speech model's memory after `idle_unload_minutes` of
    /// disuse. Off by default because the next recording pays a model reload
    /// (about 7 s warm). See `transcription::idle`.
    #[serde(default)]
    pub idle_unload_enabled: bool,
    /// Idle minutes before the model is unloaded (1-120). Only used when
    /// `idle_unload_enabled` is true.
    #[serde(default = "default_idle_unload_minutes")]
    pub idle_unload_minutes: u32,
    /// Remember recent transcripts for the history pager. In memory only: the
    /// transcripts themselves are never written to disk and are gone on quit.
    #[serde(default = "default_true")]
    pub history_enabled: bool,
    /// How many transcripts to remember (1-10). Only used when
    /// `history_enabled` is true.
    #[serde(default = "default_history_limit")]
    pub history_limit: u32,
    /// Show the floating recording pill (waveform + cancel) while recording.
    /// See `overlay`.
    #[serde(default = "default_true")]
    pub recording_overlay_enabled: bool,
    /// One of `OVERLAY_POSITIONS`; only used when the overlay is enabled.
    #[serde(default = "default_overlay_position")]
    pub recording_overlay_position: String,
    /// VAD sensitivity as a percentage (0-100): the minimum voice level that
    /// triggers recording. Deliberately NOT cloud-synced - it is tuned to a
    /// specific microphone and room.
    #[serde(default = "default_voice_threshold")]
    pub voice_threshold: u8,
    /// Whether the localhost IPC server (CLI `echophrase-cli transcribe` / MCP
    /// `transcribe` tool) should be running. Off by default so a fresh install
    /// never opens a local port.
    #[serde(default)]
    pub mcp_server_enabled: bool,
}

fn default_ui_locale() -> String {
    "en".to_string()
}

fn default_language() -> String {
    "auto".to_string()
}

fn default_chinese_script() -> String {
    "traditional".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL_ID.to_string(),
            language: default_language(),
            theme: "dark".to_string(),
            output_format: "text".to_string(),
            // macOS defaults to the dedicated Dictation (mic) key, remapped to
            // F13 at the HID layer and shown as "F5"; other platforms use
            // Ctrl+Shift+Space.
            hotkey: if cfg!(target_os = "macos") {
                "f13"
            } else {
                "ctrl+shift+space"
            }
            .to_string(),
            hotkey_aliases: HashMap::new(),
            auto_copy: true, // Deprecated - kept for backward compatibility
            output_to: vec!["paste_at_cursor".to_string()], // Default to paste at cursor
            show_timestamps: false,
            compute_type: "auto".to_string(), // Auto-detect optimal compute type
            custom_model_name: None,
            custom_model_url: None,
            custom_model_path: None,
            custom_model_format: None,
            max_recording_time: Some(30),         // Default 30 seconds
            show_hotkey_changer: true,            // Show hotkey customization UI by default
            show_realtime_preview: true,          // Show realtime transcription preview by default
            show_output_confirmations: true,      // Show output confirmation toasts by default
            show_copy_last_button: true,          // Show copy last output button by default
            show_input_changer: true,             // Show input changer by default
            show_model_switcher: true,            // Show model switcher by default
            show_transcription_speed: false,      // Don't show speed toast by default
            show_last_on_hover: true,             // Show last transcription on hover by default
            remove_filler_words: true,            // Remove filler words by default
            use_numeric_digits: false,            // Don't convert numbers to digits by default
            append_space_incomplete: true, // Append trailing space for incomplete sentences by default
            convert_spoken_punctuation: true, // Spoken "question mark" -> ?
            hotkey_mode: default_hotkey_mode(), // Tap toggles, hold+release stops
            restore_clipboard_after_paste: false, // Keep the transcript on the clipboard
            json_metadata_template: None,  // Use default JSON template
            auto_detected_model: false,    // Not auto-detected until first launch detection runs
            execution_provider: None,      // Auto-detect by default
            transform_enabled: false,      // Transform disabled by default
            transform_model: Some(DEFAULT_TRANSFORM_MODEL_ID.to_string()), // Bundled tiny tier
            transform_rules: DEFAULT_TRANSFORM_RULES.to_string(), // Default transform rules
            save_directory: None,          // No auto-save directory configured
            ui_locale: "en".to_string(),   // Default to English UI
            recording_mode: None,          // Defaults to push_to_talk
            silence_timeout: None,         // Defaults to 5 seconds in frontend
            minimize_to_tray_on_close: true, // Hide to tray on X click by default
            tray_close_notice_shown: false, // Show first-close notification once
            input_device: None,            // Use the system default input
            chinese_script: default_chinese_script(), // Traditional output by default
            ui_scale_mode: "auto".to_string(), // System default size (no extra zoom) by default
            ui_scale_manual: 1.0,          // 100% when switched to manual
            ui_scale_with_window: false,   // Opt-in window-size scaling
            idle_unload_enabled: false,    // Opt-in; keeps the model hot by default
            idle_unload_minutes: default_idle_unload_minutes(),
            history_enabled: true, // Remember recent transcripts (in memory only)
            history_limit: default_history_limit(),
            recording_overlay_enabled: true, // Floating recording pill on by default
            recording_overlay_position: default_overlay_position(),
            voice_threshold: default_voice_threshold(), // 50% VAD sensitivity
            mcp_server_enabled: false,                  // Off by default
        }
    }
}

impl AppSettings {
    pub fn load() -> Result<Self> {
        // Fast path: return cached snapshot if present.
        if let Ok(guard) = SETTINGS_CACHE.read() {
            if let Some(cached) = guard.as_ref() {
                return Ok(cached.clone());
            }
        }

        let config_path = Self::config_path()?;
        let settings = if config_path.exists() {
            let contents = fs::read_to_string(&config_path)?;
            let mut settings = serde_yml::from_str::<AppSettings>(&contents)?;
            settings.hotkey_mode = normalize_hotkey_mode(&settings.hotkey_mode);
            // One-time migration: "en" was the hardcoded default before any UI
            // existed to change it, so it was never a deliberate user choice.
            // Force-English silently mangles non-English speech on multilingual
            // models (Qwen3-ASR); "auto" restores model-side detection.
            let mut migrated = false;
            if settings.language == "en" {
                settings.language = default_language();
                migrated = true;
            }
            // "as_is" was removed as an option; fold it into the Traditional default.
            if settings.chinese_script == "as_is" {
                settings.chinese_script = default_chinese_script();
                migrated = true;
            }
            if migrated {
                settings.save()?;
            }
            settings
        } else {
            let default = Self::default();
            default.save()?;
            default
        };

        if let Ok(mut guard) = SETTINGS_CACHE.write() {
            *guard = Some(settings.clone());
        }
        Ok(settings)
    }

    /// Serialize to disk WITHOUT touching `SETTINGS_CACHE`.
    ///
    /// Split out from `save()` because `RwLock` is not reentrant: `update()`
    /// persists while already holding the write guard, so it must not call
    /// anything that re-acquires the lock.
    fn write_to_disk(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let yaml = serde_yml::to_string(self)?;
        fs::write(&config_path, yaml)?;
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        self.write_to_disk()?;

        // Refresh the cache with the just-saved value so subsequent reads
        // see the new state without another disk hit.
        if let Ok(mut guard) = SETTINGS_CACHE.write() {
            *guard = Some(self.clone());
        }

        Ok(())
    }

    /// Read the current in-memory settings.
    ///
    /// Prefer this over `load()` on read paths: `load()` exists for cold start
    /// and falls back to disk, while this is a pure cache clone once warm.
    pub fn get() -> Self {
        if let Ok(guard) = SETTINGS_CACHE.read() {
            if let Some(cached) = guard.as_ref() {
                return cached.clone();
            }
        }
        Self::load().unwrap_or_default()
    }

    /// Atomically mutate individual fields of the in-memory settings and persist.
    ///
    /// THE point of this function is that the write lock is held across
    /// mutate + persist. Every writer used to do `load()` (snapshot) -> mutate
    /// one field -> `save()` (write the WHOLE struct back), so two writers
    /// touching different fields would clobber each other: saving the UI locale
    /// reverted `model`, auto-detection reverted the settings modal, and so on.
    /// Mutating in place under one guard means a writer can only ever change
    /// the fields it actually touches.
    ///
    /// Use this for every partial update. `save()` remains for genuine
    /// whole-object writes (the settings modal replacing everything).
    ///
    /// # Example
    /// ```ignore
    /// AppSettings::update(|s| s.model = model_id.to_string())?;
    /// ```
    pub fn update<F: FnOnce(&mut AppSettings)>(f: F) -> Result<()> {
        // Ensure the cache is warm BEFORE taking the write guard - load() takes
        // the lock itself, so warming it inside the guard would deadlock.
        let _ = Self::get();

        let mut guard = SETTINGS_CACHE
            .write()
            .map_err(|e| anyhow::anyhow!("settings cache lock poisoned: {e}"))?;

        let settings = match guard.as_mut() {
            Some(s) => s,
            None => {
                // get() above could not populate it (unreadable config dir).
                // Seed defaults so the mutation still has somewhere to land.
                *guard = Some(Self::default());
                guard.as_mut().expect("just populated")
            }
        };

        f(settings);
        settings.write_to_disk()
    }

    fn config_path() -> Result<PathBuf> {
        // Test/sandbox override so automated tests never touch the real user file.
        if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV)
            && !dir.is_empty()
        {
            return Ok(PathBuf::from(dir).join("settings.yaml"));
        }
        let config_dir =
            dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?;
        Ok(config_dir.join("echophrase").join("settings.yaml"))
    }
}

#[tauri::command]
pub async fn get_settings() -> crate::error::CmdResult<AppSettings> {
    AppSettings::load().to_cmd()
}

#[tauri::command]
pub async fn save_settings(settings: AppSettings) -> crate::error::CmdResult<()> {
    // Normalize hotkey to lowercase (handles old PascalCase values like "Ctrl+Shift+Space")
    let mut settings = settings;
    settings.hotkey = settings.hotkey.to_lowercase();
    settings.hotkey_mode = normalize_hotkey_mode(&settings.hotkey_mode);
    settings.transform_model = settings.transform_model.map(sanitize_transform_model_id);
    settings.hotkey_aliases = normalize_hotkey_aliases(settings.hotkey_aliases);

    // Clamp UI scale to the supported range and normalize the mode so a
    // corrupt/hand-edited yaml can never persist an unreadable zoom.
    settings.voice_threshold = settings.voice_threshold.min(VOICE_THRESHOLD_MAX);
    settings.ui_scale_manual = settings.ui_scale_manual.clamp(UI_SCALE_MIN, UI_SCALE_MAX);
    settings.idle_unload_minutes = clamp_idle_unload_minutes(settings.idle_unload_minutes);
    settings.history_limit = clamp_history_limit(settings.history_limit);
    if settings.ui_scale_mode != "manual" {
        settings.ui_scale_mode = "auto".to_string();
    }

    settings.save().to_cmd()
}

/// Apply a partial settings update atomically.
///
/// Preferred over `save_settings` for changing individual settings: the patch
/// carries only the fields the caller touched, so it cannot revert a field
/// another writer changed concurrently.
#[tauri::command]
pub async fn update_settings_patch(
    app_handle: tauri::AppHandle,
    patch: SettingsPatch,
) -> crate::error::CmdResult<AppSettings> {
    let before = AppSettings::get();

    let mcp_toggled = patch
        .mcp_server_enabled
        .is_some_and(|v| v != before.mcp_server_enabled);

    crate::settings::AppSettings::update(|s| patch.apply_to(s)).to_cmd()?;
    let after = AppSettings::get();

    // Reconcile the IPC listener with the new setting (e.g. a hand-edited
    // settings.yaml import, not just the dedicated toggle command).
    if mcp_toggled {
        if after.mcp_server_enabled {
            if let Err(e) = crate::ipc::start() {
                log::error!("ipc: failed to start after settings patch: {e}");
            }
        } else {
            crate::ipc::stop();
        }
    }

    // Let the rest of the app (and the frontend store) resync.
    let _ = tauri::Emitter::emit(&app_handle, "settings-changed", ());

    log::info!(
        "[settings] patch applied: model={} language={} ui_locale={}",
        after.model,
        after.language,
        after.ui_locale
    );

    Ok(after)
}

/// Toggle the localhost MCP/CLI IPC server on or off.
///
#[tauri::command]
pub async fn set_mcp_server_enabled(
    app_handle: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    AppSettings::update(|s| s.mcp_server_enabled = enabled)
        .map_err(|e| format!("Failed to save mcp_server_enabled: {e}"))?;

    if enabled {
        crate::ipc::start()?;
    } else {
        crate::ipc::stop();
    }

    let _ = tauri::Emitter::emit(&app_handle, "settings-changed", ());

    Ok(())
}

/// Get the current UI locale setting
#[tauri::command]
pub async fn get_language_setting() -> crate::error::CmdResult<Option<String>> {
    let settings = AppSettings::load().to_cmd()?;
    // Return None if it's the default (English) - frontend handles default
    if settings.ui_locale == "en" {
        Ok(None)
    } else {
        Ok(Some(settings.ui_locale))
    }
}

/// Detect the system's preferred locale from OS settings.
/// Returns locale codes like "en-US", "zh-TW", "zh-CN", etc.
/// Cross-platform: Windows (GetUserDefaultLocaleName), macOS (CFLocaleCopyCurrent),
/// Linux ($LANG / $LC_ALL).
#[tauri::command]
pub fn get_system_locale() -> Option<String> {
    sys_locale::get_locale()
}

/// Save the UI locale setting
#[tauri::command]
pub async fn save_language_setting(language: String) -> crate::error::CmdResult<()> {
    // Only `ui_locale`. This used to round-trip the whole struct, so changing
    // the UI language silently reverted `model` (see AppSettings::update).
    AppSettings::update(|s| s.ui_locale = language).to_cmd()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_model_rejects_non_transform_ids() {
        for bad in ["parakeet-tdt-0.6b-v3", "t5-base-grammar", "nope"] {
            assert_eq!(
                sanitize_transform_model_id(bad.to_string()),
                DEFAULT_TRANSFORM_MODEL_ID
            );
        }
        assert_eq!(
            sanitize_transform_model_id(DEFAULT_TRANSFORM_MODEL_ID.to_string()),
            DEFAULT_TRANSFORM_MODEL_ID
        );
    }

    /// The regression test for the whole bug class.
    ///
    /// A patch that changes the UI locale must not disturb `model`. The old
    /// read-modify-write pattern failed exactly here: `save_language_setting`
    /// round-tripped the whole struct, so changing the UI language reverted the
    /// user's selected model back to whatever the stale snapshot held.
    #[test]
    fn patch_only_touches_mentioned_fields() {
        let mut settings = AppSettings {
            model: "qwen3-asr-0.6b".to_string(),
            language: "zh".to_string(),
            chinese_script: "traditional".to_string(),
            ..Default::default()
        };

        let patch: SettingsPatch = serde_json::from_str(r#"{"ui_locale":"zh-TW"}"#).unwrap();
        patch.apply_to(&mut settings);

        assert_eq!(settings.ui_locale, "zh-TW", "patched field applied");
        assert_eq!(settings.model, "qwen3-asr-0.6b", "model must survive");
        assert_eq!(settings.language, "zh", "language must survive");
        assert_eq!(settings.chinese_script, "traditional");
    }

    #[test]
    fn patch_distinguishes_absent_from_explicit_null() {
        let mut settings = AppSettings {
            save_directory: Some("D:/transcripts".to_string()),
            execution_provider: Some("directml".to_string()),
            ..Default::default()
        };

        // Absent key leaves the field alone.
        let untouched: SettingsPatch = serde_json::from_str(r#"{"model":"whisper"}"#).unwrap();
        untouched.apply_to(&mut settings);
        assert_eq!(settings.save_directory.as_deref(), Some("D:/transcripts"));

        // Explicit null clears it.
        let cleared: SettingsPatch = serde_json::from_str(r#"{"save_directory":null}"#).unwrap();
        cleared.apply_to(&mut settings);
        assert_eq!(settings.save_directory, None, "explicit null clears");
        assert_eq!(
            settings.execution_provider.as_deref(),
            Some("directml"),
            "unmentioned nullable field survives"
        );
    }

    #[test]
    fn patch_normalizes_like_save_settings() {
        let mut settings = AppSettings::default();

        let patch: SettingsPatch = serde_json::from_str(
            r#"{"hotkey":"Ctrl+Shift+Space","ui_scale_manual":9.0,"ui_scale_mode":"bogus"}"#,
        )
        .unwrap();
        patch.apply_to(&mut settings);

        assert_eq!(settings.hotkey, "ctrl+shift+space", "hotkey lowercased");
        assert_eq!(settings.ui_scale_manual, UI_SCALE_MAX, "scale clamped");
        assert_eq!(settings.ui_scale_mode, "auto", "unknown mode falls back");
    }

    #[test]
    fn legacy_telemetry_enabled_key_is_ignored_on_load() {
        // Pre-removal settings.yaml files still carry `telemetry_enabled`.
        let settings: AppSettings =
            serde_yml::from_str("model: parakeet-tdt-0.6b-v3\ntelemetry_enabled: true\n").unwrap();
        assert_eq!(settings.model, "parakeet-tdt-0.6b-v3");
        let yaml = serde_yml::to_string(&settings).unwrap();
        assert!(!yaml.contains("telemetry"), "key is dropped on next save");
    }

    #[test]
    fn hotkey_mode_is_normalized() {
        assert_eq!(normalize_hotkey_mode("Toggle"), "toggle");
        assert_eq!(normalize_hotkey_mode("hold"), "hold");
        assert_eq!(normalize_hotkey_mode("hybrid"), "hybrid");
        assert_eq!(normalize_hotkey_mode("bogus"), "hybrid");

        let mut settings = AppSettings::default();
        let patch: SettingsPatch = serde_json::from_str(r#"{"hotkey_mode":"HOLD"}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(settings.hotkey_mode, "hold");
        let patch: SettingsPatch = serde_json::from_str(r#"{"hotkey_mode":"nope"}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(settings.hotkey_mode, "hybrid");
    }

    #[test]
    fn patch_sanitizes_transform_model() {
        let mut settings = AppSettings::default();
        let patch: SettingsPatch =
            serde_json::from_str(r#"{"transform_model":"parakeet-tdt-0.6b-v3"}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(
            settings.transform_model.as_deref(),
            Some(DEFAULT_TRANSFORM_MODEL_ID)
        );
        let patch: SettingsPatch = serde_json::from_str(r#"{"transform_model":null}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(settings.transform_model, None);
    }

    #[test]
    fn convert_spoken_punctuation_defaults_on_and_patches() {
        // Pre-upgrade settings.yaml without the key must default ON
        let mut settings: AppSettings =
            serde_yml::from_str("model: parakeet-tdt-0.6b-v3\n").unwrap();
        assert!(settings.convert_spoken_punctuation);
        assert!(AppSettings::default().convert_spoken_punctuation);

        let patch: SettingsPatch =
            serde_json::from_str(r#"{"convert_spoken_punctuation": false}"#).unwrap();
        patch.apply_to(&mut settings);
        assert!(!settings.convert_spoken_punctuation);

        let yaml = serde_yml::to_string(&settings).unwrap();
        let reloaded: AppSettings = serde_yml::from_str(&yaml).unwrap();
        assert!(!reloaded.convert_spoken_punctuation);
    }

    #[test]
    fn ui_scale_fields_default_when_missing() {
        // Simulates a pre-upgrade settings.yaml with no ui_scale_* keys
        let settings: AppSettings = serde_yml::from_str("model: parakeet-tdt-0.6b-v3\n").unwrap();
        assert_eq!(settings.ui_scale_mode, "auto");
        assert_eq!(settings.ui_scale_manual, 1.0);
        assert!(!settings.ui_scale_with_window);
    }

    /// Every model id the registry can select must survive a disk round-trip.
    /// `deserialize_model` silently rewrites anything it doesn't recognise to
    /// DEFAULT_MODEL_ID, so a missing prefix arm makes a persisted selection
    /// revert on the next launch with no error anywhere (qwen3 did exactly
    /// this: auto-detect picked it, and every restart loaded Parakeet).
    #[test]
    fn known_model_ids_survive_deserialization() {
        for id in [
            "parakeet-tdt-0.6b-v3",
            "parakeet-eou-120m",
            "qwen3-asr-0.6b",
            "custom",
        ] {
            let settings: AppSettings = serde_yml::from_str(&format!("model: {}\n", id)).unwrap();
            assert_eq!(settings.model, id, "model id '{}' was not preserved", id);
        }
    }

    /// Genuinely unknown ids still fall back to the default rather than
    /// leaving the app pointed at a model that cannot load.
    #[test]
    fn unknown_model_id_falls_back_to_default() {
        let settings: AppSettings = serde_yml::from_str("model: not-a-real-model\n").unwrap();
        assert_eq!(settings.model, DEFAULT_MODEL_ID);
    }

    /// A persisted selection from the removed hosted-transcription scaffold
    /// must not break startup: it falls back to the default like any unknown id.
    #[test]
    fn legacy_hosted_model_id_falls_back_to_default() {
        let settings: AppSettings =
            serde_yml::from_str("model: hosted-whisper\nhosted_api_key: sk-old\n").unwrap();
        assert_eq!(settings.model, DEFAULT_MODEL_ID);
    }

    #[test]
    fn idle_unload_is_off_by_default_with_ten_minutes() {
        let defaults = AppSettings::default();
        assert!(!defaults.idle_unload_enabled);
        assert_eq!(defaults.idle_unload_minutes, 10);

        // Pre-upgrade settings.yaml with neither key
        let settings: AppSettings = serde_yml::from_str("model: parakeet-tdt-0.6b-v3\n").unwrap();
        assert!(!settings.idle_unload_enabled);
        assert_eq!(settings.idle_unload_minutes, 10);
    }

    #[test]
    fn idle_unload_round_trips() {
        let settings = AppSettings {
            idle_unload_enabled: true,
            idle_unload_minutes: 25,
            ..Default::default()
        };
        let yaml = serde_yml::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_yml::from_str(&yaml).unwrap();
        assert!(parsed.idle_unload_enabled);
        assert_eq!(parsed.idle_unload_minutes, 25);
    }

    #[test]
    fn idle_unload_patch_clamps_minutes() {
        let mut settings = AppSettings::default();
        let patch: SettingsPatch =
            serde_json::from_str(r#"{"idle_unload_enabled": true, "idle_unload_minutes": 99999}"#)
                .unwrap();
        patch.apply_to(&mut settings);
        assert!(settings.idle_unload_enabled);
        assert_eq!(settings.idle_unload_minutes, 120);

        let patch: SettingsPatch = serde_json::from_str(r#"{"idle_unload_minutes": 0}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(settings.idle_unload_minutes, 1);
    }

    #[test]
    fn history_is_on_by_default_with_ten() {
        let defaults = AppSettings::default();
        assert!(defaults.history_enabled);
        assert_eq!(defaults.history_limit, 10);

        // Pre-upgrade settings.yaml with neither key
        let settings: AppSettings = serde_yml::from_str("model: parakeet-tdt-0.6b-v3\n").unwrap();
        assert!(settings.history_enabled);
        assert_eq!(settings.history_limit, 10);
    }

    #[test]
    fn history_patch_toggles_and_clamps_limit() {
        let mut settings = AppSettings::default();
        let patch: SettingsPatch =
            serde_json::from_str(r#"{"history_enabled": false, "history_limit": 99}"#).unwrap();
        patch.apply_to(&mut settings);
        assert!(!settings.history_enabled);
        assert_eq!(settings.history_limit, 10);

        let patch: SettingsPatch = serde_json::from_str(r#"{"history_limit": 0}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(settings.history_limit, 1);
    }

    #[test]
    fn recording_overlay_defaults_on_and_patches() {
        let settings: AppSettings = serde_yml::from_str(
            "model: parakeet-tdt-0.6b-v3
",
        )
        .unwrap();
        assert!(settings.recording_overlay_enabled);

        let mut settings = AppSettings::default();
        let patch: SettingsPatch =
            serde_json::from_str(r#"{"recording_overlay_enabled": false}"#).unwrap();
        patch.apply_to(&mut settings);
        assert!(!settings.recording_overlay_enabled);
        assert_eq!(settings.recording_overlay_position, "bottom-center");

        let patch: SettingsPatch =
            serde_json::from_str(r#"{"recording_overlay_position": "top-left"}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(settings.recording_overlay_position, "top-left");

        let patch: SettingsPatch =
            serde_json::from_str(r#"{"recording_overlay_position": "middle"}"#).unwrap();
        patch.apply_to(&mut settings);
        assert_eq!(settings.recording_overlay_position, "bottom-center");
    }

    #[test]
    fn ui_scale_fields_round_trip() {
        let settings = AppSettings {
            ui_scale_mode: "manual".to_string(),
            ui_scale_manual: 1.5,
            ui_scale_with_window: true,
            ..Default::default()
        };

        let yaml = serde_yml::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_yml::from_str(&yaml).unwrap();
        assert_eq!(parsed.ui_scale_mode, "manual");
        assert_eq!(parsed.ui_scale_manual, 1.5);
        assert!(parsed.ui_scale_with_window);
    }

    #[test]
    fn ui_scale_clamp_bounds() {
        assert_eq!(2.5_f64.clamp(UI_SCALE_MIN, UI_SCALE_MAX), 2.0);
        assert_eq!(0.1_f64.clamp(UI_SCALE_MIN, UI_SCALE_MAX), 0.5);
        assert_eq!(1.5_f64.clamp(UI_SCALE_MIN, UI_SCALE_MAX), 1.5);
    }
}

#[cfg(test)]
mod patch_tests {
    use super::*;

    /// Every key the frontend can send must have a field here. Before
    /// `deny_unknown_fields`, a missing field was dropped in silence: the patch
    /// "succeeded", the value never reached disk, and the setting reverted on the next
    /// load. That is exactly how `voice_threshold` went unpersisted. This test pins the
    /// loud behaviour so the failure surfaces at the call site instead.
    #[test]
    fn unknown_patch_keys_are_rejected_not_silently_dropped() {
        let err = serde_json::from_str::<SettingsPatch>(r#"{"not_a_real_setting":40}"#)
            .expect_err("unknown keys must be rejected");

        assert!(
            err.to_string().contains("not_a_real_setting"),
            "error should name the offending key, got: {err}"
        );
    }

    /// A patch mentioning only some fields still parses - partial updates are the
    /// whole point of the patch path.
    #[test]
    fn partial_patch_still_parses() {
        let patch: SettingsPatch =
            serde_json::from_str(r#"{"theme":"light","voice_threshold":40}"#).unwrap();

        assert_eq!(Some("light".to_string()), patch.theme);
        assert_eq!(Some(40), patch.voice_threshold);
    }

    /// voice_threshold persists through the patch path and is clamped to 0-100.
    #[test]
    fn voice_threshold_applies_and_clamps() {
        let mut settings = AppSettings::default();

        SettingsPatch {
            voice_threshold: Some(40),
            ..Default::default()
        }
        .apply_to(&mut settings);
        assert_eq!(40, settings.voice_threshold);

        SettingsPatch {
            voice_threshold: Some(255),
            ..Default::default()
        }
        .apply_to(&mut settings);
        assert_eq!(VOICE_THRESHOLD_MAX, settings.voice_threshold);
    }

    /// A settings file written by an older build still carries the removed
    /// tier / cloud-sync keys. Loading it must succeed (AppSettings ignores unknown
    /// keys) and the patch path must tolerate the legacy cloud-sync keys too.
    #[test]
    fn legacy_settings_json_with_removed_keys_still_loads() {
        let old = r#"{
            "theme": "light",
            "voice_threshold": 40,
            "cloud_sync_enabled": true,
            "pending_cloud_model": "parakeet-tdt-0.6b-v3",
            "pending_cloud_transform_model": null,
            "hosted_api_key": "sk-legacy",
            "tier": "pro",
            "trial_started_at": "2026-01-01T00:00:00Z",
            "auth_session": {"access_token": "x"}
        }"#;

        let settings: AppSettings = serde_json::from_str(old).expect("old settings must load");
        assert_eq!("light", settings.theme);
        assert_eq!(40, settings.voice_threshold);

        let patch: SettingsPatch = serde_json::from_str(
            r#"{"theme":"dark","cloud_sync_enabled":true,"pending_cloud_model":null,"hosted_api_key":null}"#,
        )
        .expect("legacy cloud-sync keys must not fail a patch");
        assert_eq!(Some("dark".to_string()), patch.theme);
    }
}
