// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! [`AudioSource`] - the single switch point between audio that lives in a
//! FILE on disk and audio already in MEMORY.
//!
//! Every consumer (transcription backends, the CLI, the future MCP server)
//! resolves input through this enum instead of choosing its own I/O strategy.
//! Live capture produces `Memory`; file-drop/explicit paths produce `File`;
//! downstream code cannot tell the difference.

use std::borrow::Cow;
use std::path::PathBuf;

use crate::{Result, TARGET_SAMPLE_RATE, resample, sample, wav};

/// Mono `f32` samples plus the rate they were captured/decoded at.
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Mono samples, normalized to `[-1.0, 1.0]`.
    pub samples: Vec<f32>,
    /// Sample rate of `samples` in Hz.
    pub sample_rate: u32,
}

impl AudioBuffer {
    /// Wrap samples that are ALREADY at the canonical 16 kHz rate.
    pub fn new_16k(samples: Vec<f32>) -> Self {
        Self {
            samples,
            sample_rate: TARGET_SAMPLE_RATE,
        }
    }

    /// Consume the buffer, resampling to 16 kHz only if needed.
    pub fn into_mono_16k(self) -> Vec<f32> {
        if self.sample_rate == TARGET_SAMPLE_RATE {
            self.samples
        } else {
            resample::resample_linear(&self.samples, self.sample_rate, TARGET_SAMPLE_RATE)
        }
    }

    /// Duration in whole milliseconds.
    pub fn duration_ms(&self) -> u32 {
        sample::duration_ms(&self.samples, self.sample_rate)
    }
}

/// Where a piece of audio comes from: a file on disk or an in-memory buffer.
#[derive(Debug, Clone)]
pub enum AudioSource {
    /// A WAV file on disk (file-drop, explicit path, CLI input).
    File(PathBuf),
    /// Samples already in memory (live capture, upstream processing).
    Memory(AudioBuffer),
}

impl AudioSource {
    /// Source backed by a file path.
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }

    /// Source backed by in-memory samples already at 16 kHz mono.
    pub fn from_samples_16k(samples: Vec<f32>) -> Self {
        Self::Memory(AudioBuffer::new_16k(samples))
    }

    /// Resolve to the canonical pipeline format: 16 kHz mono `f32`.
    ///
    /// `File` decodes + downmixes + resamples; `Memory` resamples only if its
    /// rate differs. An already-16k memory buffer is borrowed, not copied.
    pub fn samples_mono_16k(&self) -> Result<Cow<'_, [f32]>> {
        match self {
            Self::File(path) => Ok(Cow::Owned(wav::read_wav_mono_16k(path)?)),
            Self::Memory(buf) if buf.sample_rate == TARGET_SAMPLE_RATE => {
                Ok(Cow::Borrowed(&buf.samples))
            }
            Self::Memory(buf) => Ok(Cow::Owned(resample::resample_linear(
                &buf.samples,
                buf.sample_rate,
                TARGET_SAMPLE_RATE,
            ))),
        }
    }

    /// Canonical WAV bytes (16 kHz mono 32-bit float).
    ///
    /// Files pass through decode -> encode so consumers always receive the same
    /// format regardless of what was on disk.
    pub fn to_wav_bytes(&self) -> Result<Vec<u8>> {
        wav::encode_wav_f32_mono_16k(&self.samples_mono_16k()?)
    }

    /// Duration in whole milliseconds (decodes `File` sources to count).
    pub fn duration_ms(&self) -> Result<u32> {
        match self {
            Self::File(_) => {
                let samples = self.samples_mono_16k()?;
                Ok(sample::duration_ms(&samples, TARGET_SAMPLE_RATE))
            }
            Self::Memory(buf) => Ok(buf.duration_ms()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// The core guarantee: the SAME audio via File and via Memory resolves to
    /// the same pipeline samples.
    #[test]
    fn file_and_memory_sources_are_equivalent() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("src.wav");
        // 48 kHz mono so the File path exercises resampling too.
        let samples_48k: Vec<f32> = (0..4800).map(|i| ((i as f32) * 0.01).sin() * 0.5).collect();
        wav::write_wav_i16_mono(&path, 48_000, &samples_48k).unwrap();

        let from_file = AudioSource::from_path(&path)
            .samples_mono_16k()
            .unwrap()
            .into_owned();

        // Memory source: decode the same file, then resolve through Memory.
        let decoded = wav::read_wav(&path).unwrap();
        let from_memory = AudioSource::Memory(decoded)
            .samples_mono_16k()
            .unwrap()
            .into_owned();

        assert_eq!(from_file, from_memory);
        assert_eq!(from_file.len(), 1600); // 0.1s at 16k
    }

    #[test]
    fn memory_16k_borrows_without_copy() {
        let src = AudioSource::from_samples_16k(vec![0.1, 0.2]);
        match src.samples_mono_16k().unwrap() {
            Cow::Borrowed(s) => assert_eq!(s, &[0.1, 0.2]),
            Cow::Owned(_) => panic!("16k memory source should borrow"),
        }
    }

    #[test]
    fn memory_non_16k_resamples() {
        let buf = AudioBuffer {
            samples: vec![0.0; 4800],
            sample_rate: 48_000,
        };
        let src = AudioSource::Memory(buf);
        assert_eq!(src.samples_mono_16k().unwrap().len(), 1600);
        assert_eq!(src.duration_ms().unwrap(), 100);
    }

    #[test]
    fn wav_bytes_from_memory_source() {
        let src = AudioSource::from_samples_16k(vec![0.25, -0.25]);
        let bytes = src.to_wav_bytes().unwrap();
        let reader = hound::WavReader::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(reader.spec().channels, 1);
    }

    #[test]
    fn missing_file_is_an_error() {
        let src = AudioSource::from_path("/definitely/not/here.wav");
        assert!(src.samples_mono_16k().is_err());
    }
}
