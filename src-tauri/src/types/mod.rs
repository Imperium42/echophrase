// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use serde::{Deserialize, Serialize};
use std::fmt;

/// Download status enum with proper serialization
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Connecting,
    Downloading,
    VerifyingIntegrity,
    Complete,
    Error,
    Cancelled,
}

impl fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connecting => write!(f, "Connecting..."),
            Self::Downloading => write!(f, "Downloading"),
            Self::VerifyingIntegrity => write!(f, "Verifying integrity"),
            Self::Complete => write!(f, "Complete"),
            Self::Error => write!(f, "Error"),
            Self::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Audio quality levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioQuality {
    Low,
    Medium,
    High,
    Ultra,
}

#[allow(dead_code)]
impl AudioQuality {
    pub fn sample_rate(&self) -> u32 {
        match self {
            Self::Low => 8000,
            Self::Medium => 16000,
            Self::High => 44100,
            Self::Ultra => 48000,
        }
    }

    pub fn bit_depth(&self) -> u16 {
        match self {
            Self::Low => 8,
            Self::Medium => 16,
            Self::High => 16,
            Self::Ultra => 24,
        }
    }

    pub fn channels(&self) -> u16 {
        match self {
            Self::Low | Self::Medium | Self::High => 1,
            Self::Ultra => 2,
        }
    }
}

/// Recording status states
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Idle,
    Initializing,
    Recording,
    Paused,
    Processing,
    Complete,
    Error,
}

/// Error severity levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Error categories for better handling
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Network,
    FileSystem,
    Audio,
    Model,
    Transcription,
    Settings,
    Permission,
    Unknown,
}

/// Application error with proper categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    pub id: String,
    pub category: ErrorCategory,
    pub severity: ErrorSeverity,
    pub message: String,
    pub details: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub recoverable: bool,
}

#[allow(dead_code)]
impl AppError {
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            category,
            severity: ErrorSeverity::Error,
            message: message.into(),
            details: None,
            timestamp: chrono::Utc::now(),
            recoverable: true,
        }
    }

    pub fn with_severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn non_recoverable(mut self) -> Self {
        self.recoverable = false;
        self
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.category, self.message)?;
        if let Some(ref details) = self.details {
            write!(f, " ({})", details)?;
        }
        Ok(())
    }
}

impl std::error::Error for AppError {}

/// Model download progress with proper types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDownloadProgress {
    pub progress: u8, // 0-100
    pub speed: String,
    pub time_remaining: String,
    pub time_elapsed: String,
    pub status: DownloadStatus,
}

/// Audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
    pub quality: AudioQuality,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            bit_depth: 16,
            quality: AudioQuality::Medium,
        }
    }
}

/// Audio normalization configuration for Whisper preprocessing
///
/// Based on industry standards and research:
/// - Whisper expects PCM audio in [-1, 1] range
/// - Audio below -20dBFS causes transcription issues
/// - Standard speech levels: ~-20dBFS RMS, -3dBFS peak
/// - Whisper is NOT scale-invariant (trained with specific amplitude ranges)
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct AudioNormalizationConfig {
    /// Target peak level for normalization (0.0-1.0)
    /// Default: 0.8 (~-2dBFS, leaves safe headroom for dynamics)
    /// Audio below this level will be boosted to reach target_peak
    pub target_peak: f32,

    /// Required sample rate for Whisper model
    /// Whisper requires 16kHz mono PCM
    pub sample_rate: u32,
}

impl Default for AudioNormalizationConfig {
    fn default() -> Self {
        Self {
            target_peak: 0.8,   // -2dBFS (20 * log10(0.8) ≈ -1.94dBFS)
            sample_rate: 16000, // Whisper requirement
        }
    }
}

impl AudioNormalizationConfig {
    /// Normalize PCM audio samples to optimal level for Whisper transcription
    ///
    /// - Boosts audio below target_peak to reach target_peak level
    /// - Prevents clipping by scaling down audio > 1.0
    /// - Only processes audio that needs adjustment
    pub fn normalize(&self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }

        // Find peak absolute value
        let max_abs = samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);

        // Avoid division by zero for silent audio
        if max_abs < 1e-10 {
            return;
        }

        if max_abs < self.target_peak {
            // Audio below target - boost to target peak
            let gain = self.target_peak / max_abs;
            for sample in samples {
                *sample *= gain;
            }
        } else if max_abs > 1.0 {
            // Audio too loud - prevent clipping
            let gain = 0.99 / max_abs; // Scale to just below 1.0
            for sample in samples {
                *sample *= gain;
            }
        }
        // else: audio already at optimal level (between target_peak and 1.0)
    }
}

/// Constants for audio processing
#[allow(dead_code)]
pub mod audio_constants {
    pub const DEFAULT_MAX_RECORDING_DURATION_SECS: u64 = 30; // Default 30 seconds
    pub const ABSOLUTE_MIN_RECORDING_DURATION_SECS: u64 = 5; // Minimum 5 seconds
    pub const ABSOLUTE_MAX_RECORDING_DURATION_SECS: u64 = 180; // Maximum 180 seconds (3 minutes)
    pub const MIN_RECORDING_DURATION_SECS: f32 = 0.5; // For silence detection
    pub const MAX_FILE_SIZE_MB: usize = 100;
    pub const SILENCE_THRESHOLD_DB: f32 = -50.0;
    pub const NOISE_FLOOR_DB: f32 = -60.0;
    pub const PEAK_WARNING_DB: f32 = -3.0;
    pub const CLIPPING_THRESHOLD_DB: f32 = 0.0;
}

/// Constants for model operations
#[allow(dead_code)]
pub mod model_constants {
    pub const DEFAULT_BEAM_SIZE: u8 = 1; // Greedy decoding
    #[allow(dead_code)]
    pub const DEFAULT_PATIENCE: f32 = 1.0;
    #[allow(dead_code)]
    pub const DEFAULT_LENGTH_PENALTY: f32 = 1.0;
    #[allow(dead_code)]
    pub const MODEL_CACHE_DIR: &str = ".cache/huggingface/hub";
}
