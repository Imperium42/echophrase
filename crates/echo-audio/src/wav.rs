// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! WAV I/O via `hound`: read any PCM WAV into the canonical mono-f32 form,
//! write 16-bit mono WAVs, and encode in-memory WAV bytes for network
//! providers.
// TODO: WAV-only today. Broaden to compressed inputs (mp3/m4a/flac/ogg) via
// `symphonia` so users can transcode arbitrary voice files. See cli/TODO.md.

use std::io::Cursor;
use std::path::Path;

use crate::source::AudioBuffer;
use crate::{Result, TARGET_SAMPLE_RATE, sample};

/// Read a WAV file into mono `f32` samples at its ORIGINAL sample rate.
///
/// Accepts integer PCM of any bit depth and 32-bit float; multi-channel input
/// is downmixed to mono. Resampling is the caller's decision (see
/// [`read_wav_mono_16k`] or [`AudioBuffer::into_mono_16k`]).
pub fn read_wav(path: &Path) -> Result<AudioBuffer> {
    let reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples = decode_samples(reader, spec)?;
    let mono = sample::downmix_to_mono(&samples, spec.channels);
    Ok(AudioBuffer {
        samples: mono,
        sample_rate: spec.sample_rate,
    })
}

/// Read a WAV file straight into the canonical pipeline format:
/// 16 kHz mono `f32`. (Read + downmix + resample in one call.)
pub fn read_wav_mono_16k(path: &Path) -> Result<Vec<f32>> {
    Ok(read_wav(path)?.into_mono_16k())
}

/// Write mono `f32` samples as a 16-bit integer PCM WAV file, creating parent
/// directories as needed.
pub fn write_wav_i16_mono(path: &Path, sample_rate: u32, samples: &[f32]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut writer = hound::WavWriter::create(path, i16_mono_spec(sample_rate))?;
    for &s in samples {
        writer.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Encode 16 kHz mono `f32` samples as 32-bit float WAV bytes in memory -
/// a self-contained WAV payload for in-memory consumers.
pub fn encode_wav_f32_mono_16k(samples: &[f32]) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: TARGET_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for &s in samples {
            writer.write_sample(s)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

/// Decode a WAV reader's samples to `f32` in `[-1.0, 1.0]`, whatever the
/// on-disk format (integer of any bit depth, or float).
fn decode_samples<R: std::io::Read>(
    reader: hound::WavReader<R>,
    spec: hound::WavSpec,
) -> Result<Vec<f32>> {
    let samples = match spec.sample_format {
        hound::SampleFormat::Int => {
            let max_val = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<std::result::Result<Vec<f32>, _>>()?
        }
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<std::result::Result<Vec<f32>, _>>()?,
    };
    Ok(samples)
}

/// The 16-bit mono integer spec used for all WAV files this crate writes.
fn i16_mono_spec(sample_rate: u32) -> hound::WavSpec {
    hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_then_read_roundtrips_within_quantization() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("rt.wav");
        let samples = vec![0.0f32, 0.5, -0.5, 0.999, -0.999];
        write_wav_i16_mono(&path, TARGET_SAMPLE_RATE, &samples).unwrap();

        let buf = read_wav(&path).unwrap();
        assert_eq!(buf.sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(buf.samples.len(), samples.len());
        for (orig, got) in samples.iter().zip(buf.samples.iter()) {
            // Write scales by 32767 and truncates; read divides by 32768. The
            // combined round-trip error stays within ~2 LSB of 16-bit.
            assert!((orig - got).abs() < 2.0 / 32767.0);
        }
    }

    #[test]
    fn read_stereo_downmixes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("stereo.wav");
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for &(l, r) in &[(16384i16, 0i16), (0, -16384)] {
            w.write_sample(l).unwrap();
            w.write_sample(r).unwrap();
        }
        w.finalize().unwrap();

        let buf = read_wav(&path).unwrap();
        assert_eq!(buf.sample_rate, 48_000);
        assert_eq!(buf.samples.len(), 2);
        assert!((buf.samples[0] - 0.25).abs() < 1e-3);
        assert!((buf.samples[1] + 0.25).abs() < 1e-3);
    }

    #[test]
    fn read_float_wav() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("float.wav");
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for &s in &[0.25f32, -0.75] {
            w.write_sample(s).unwrap();
        }
        w.finalize().unwrap();

        let buf = read_wav(&path).unwrap();
        assert_eq!(buf.samples, vec![0.25, -0.75]);
    }

    #[test]
    fn encode_wav_bytes_parse_back() {
        let samples = vec![0.1f32, -0.2, 0.3];
        let bytes = encode_wav_f32_mono_16k(&samples).unwrap();
        let reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);
        let back: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(back, samples);
    }
}
