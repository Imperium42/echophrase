// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Microphone capture to a 16 kHz, 16-bit, mono WAV file.
//!
//! All audio logic lives in the shared [`echo_audio`] crate; this module is a
//! thin composition: capture chunks in memory until the shared `stop` flag is
//! set, then write the WAV once. Output is always **16 kHz mono** - the format
//! Echophrase's transcription pipeline expects.
//!
//! Two backends sit behind the `mic` Cargo feature:
//!
//! * `mic` (default): real capture via `echo_audio::capture` (cpal across
//!   WASAPI / CoreAudio / ALSA).
//! * stub (when built `--no-default-features`): writes silence at 16 kHz so
//!   the record/stop/session flow can be exercised without audio hardware.

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Sample rate every recording is written at (Echophrase pipeline requirement).
pub const TARGET_RATE: u32 = echo_audio::TARGET_SAMPLE_RATE;

/// Details of the recording that was written.
pub struct RecordingInfo {
    /// Sample rate of the written file (always [`TARGET_RATE`]).
    pub sample_rate: u32,
    /// Rate the hardware actually captured at (before resampling).
    pub source_rate: u32,
}

/// Record from `device_name` (or the default input) into `output` until `stop`
/// is set to `true`. Output is always 16 kHz mono.
pub fn capture_to_wav(
    output: &Path,
    device_name: Option<&str>,
    stop: Arc<AtomicBool>,
) -> Result<RecordingInfo> {
    #[cfg(feature = "mic")]
    {
        mic::capture(output, device_name, stop)
    }
    #[cfg(not(feature = "mic"))]
    {
        stub::capture(output, device_name, stop)
    }
}

#[cfg(feature = "mic")]
mod mic {
    use super::{RecordingInfo, TARGET_RATE};
    use anyhow::{Context, Result, bail};
    use echo_audio::capture::{CaptureConfig, CaptureEvent, start_capture};
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    /// Chunk size for plain recording (64 ms at 16 kHz) - small enough to stop
    /// promptly, large enough to keep channel traffic negligible.
    const CHUNK_SAMPLES: usize = 1024;

    pub fn capture(
        output: &Path,
        device_name: Option<&str>,
        stop: Arc<AtomicBool>,
    ) -> Result<RecordingInfo> {
        let (handle, events) = start_capture(CaptureConfig {
            device_name: device_name.map(String::from),
            chunk_samples: CHUNK_SAMPLES,
        })
        .context("starting capture")?;
        let source_rate = handle.source_rate();

        // Accumulate chunks (already 16 kHz mono) until asked to stop.
        let mut samples: Vec<f32> = Vec::new();
        while !stop.load(Ordering::SeqCst) {
            match events.recv_timeout(Duration::from_millis(100)) {
                Ok(CaptureEvent::Chunk(chunk)) => samples.extend_from_slice(&chunk),
                Ok(CaptureEvent::Error(e)) => bail!("audio stream error: {e}"),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }

        // Stop the stream, then drain the tail the capture thread flushed.
        handle.stop();
        for event in events.try_iter() {
            if let CaptureEvent::Chunk(chunk) = event {
                samples.extend_from_slice(&chunk);
            }
        }

        echo_audio::wav::write_wav_i16_mono(output, TARGET_RATE, &samples)
            .context("writing wav")?;
        Ok(RecordingInfo {
            sample_rate: TARGET_RATE,
            source_rate,
        })
    }
}

#[cfg(not(feature = "mic"))]
mod stub {
    use super::{RecordingInfo, TARGET_RATE};
    use anyhow::{Context, Result};
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    pub fn capture(
        output: &Path,
        _device_name: Option<&str>,
        stop: Arc<AtomicBool>,
    ) -> Result<RecordingInfo> {
        eprintln!(
            "[echophrase] built without the 'mic' feature: writing silence (no microphone capture)."
        );

        // Roughly real-time so the file duration tracks wall-clock recording.
        let chunk = TARGET_RATE as usize / 10; // 0.1s of samples
        let mut samples: Vec<f32> = Vec::new();
        while !stop.load(Ordering::SeqCst) {
            samples.extend(std::iter::repeat_n(0.0f32, chunk));
            std::thread::sleep(Duration::from_millis(100));
        }

        echo_audio::wav::write_wav_i16_mono(output, TARGET_RATE, &samples)
            .context("writing wav")?;
        Ok(RecordingInfo {
            sample_rate: TARGET_RATE,
            source_rate: TARGET_RATE,
        })
    }
}
