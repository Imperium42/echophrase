// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Echophrase audio core - the single source of truth for audio handling.
//!
//! Everything that touches raw audio in the Echophrase workspace lives here:
//!
//! * [`sample`]   - pure sample math (downmix, normalize, peak/RMS, conversions)
//! * [`resample`] - linear resampling, batch and streaming
//! * [`wav`]      - WAV file/byte I/O (hound)
//! * [`source`]   - [`AudioSource`]: the one switch point between an audio FILE
//!   on disk and an in-MEMORY buffer
//! * [`capture`]  - live microphone capture via cpal (feature `capture`)
//!
//! The canonical pipeline format is **16 kHz, mono, f32 in [-1.0, 1.0]**
//! ([`TARGET_SAMPLE_RATE`]). Consumers resolve any input through
//! [`AudioSource::samples_mono_16k`] and never re-implement decode/resample.

pub mod resample;
pub mod sample;
pub mod source;
pub mod wav;
pub mod wsl;

#[cfg(feature = "capture")]
pub mod capture;

pub use source::{AudioBuffer, AudioSource};

/// Sample rate the transcription pipeline expects everywhere.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Peak level live/normalized audio is scaled to (-1.94 dBFS).
pub const NORMALIZE_TARGET_PEAK: f32 = 0.8;

/// Below this peak a signal is treated as silence and never amplified.
pub const NORMALIZE_MIN_THRESHOLD: f32 = 1e-10;

/// Errors produced by this crate.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("wav error: {0}")]
    Wav(#[from] hound::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Capture(String),
    #[error("{0}")]
    InvalidInput(String),
}

/// Crate-wide result alias.
pub type Result<T> = std::result::Result<T, AudioError>;
