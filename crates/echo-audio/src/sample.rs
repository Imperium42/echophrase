// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Pure sample math. No I/O, no platform audio - just `&[f32]` in, values out.
//!
//! Samples throughout this crate are mono (unless a `channels` parameter says
//! otherwise) `f32` normalized to `[-1.0, 1.0]`.

/// Average interleaved frames of `channels` samples down to a single mono track.
///
/// Mono input (`channels <= 1`) passes through untouched. A trailing partial
/// frame is averaged over the samples it actually has.
// TODO: naive channel averaging; consider proper downmix weighting (e.g.
// center/LFE coefficients) for >2 channel sources. See cli/TODO.md.
pub fn downmix_to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let frame = channels.max(1) as usize;
    if frame == 1 {
        return samples.to_vec();
    }
    samples
        .chunks(frame)
        .map(|c| c.iter().copied().sum::<f32>() / c.len() as f32)
        .collect()
}

/// Return a copy of `samples` scaled so the peak sits at `target_peak`.
///
/// Near-silent input (peak below [`crate::NORMALIZE_MIN_THRESHOLD`]) is returned
/// unchanged so a noise floor is never amplified.
pub fn normalize_peak(samples: &[f32], target_peak: f32) -> Vec<f32> {
    let mut out = samples.to_vec();
    normalize_peak_in_place(&mut out, target_peak);
    out
}

/// In-place variant of [`normalize_peak`] for hot paths that own their buffer.
pub fn normalize_peak_in_place(samples: &mut [f32], target_peak: f32) {
    let current = peak(samples);
    if current < crate::NORMALIZE_MIN_THRESHOLD {
        return;
    }
    let scale = target_peak / current;
    for s in samples.iter_mut() {
        *s *= scale;
    }
}

/// Peak absolute amplitude of `samples` (0.0 for empty input).
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
}

/// Root-mean-square level of `samples` (0.0 for empty input).
pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Convert little-endian signed 16-bit PCM bytes to normalized `f32` samples.
///
/// A trailing odd byte (should never happen in valid PCM) is ignored.
pub fn i16_bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c) as f32 / 32768.0)
        .collect()
}

/// Duration of `samples` at `sample_rate`, in whole milliseconds.
pub fn duration_ms(samples: &[f32], sample_rate: u32) -> u32 {
    if sample_rate == 0 {
        return 0;
    }
    ((samples.len() as u64) * 1000 / sample_rate as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NORMALIZE_TARGET_PEAK;

    #[test]
    fn downmix_mono_is_passthrough() {
        let s = vec![0.1, 0.2, 0.3];
        assert_eq!(downmix_to_mono(&s, 1), s);
        assert_eq!(downmix_to_mono(&s, 0), s); // degenerate channel count
    }

    #[test]
    fn downmix_stereo_averages_pairs() {
        let s = vec![1.0, 0.0, 0.5, -0.5];
        assert_eq!(downmix_to_mono(&s, 2), vec![0.5, 0.0]);
    }

    #[test]
    fn downmix_quad_averages_frames() {
        let s = vec![1.0, 1.0, 1.0, 1.0, 0.0, 0.2, 0.4, 0.6];
        let out = downmix_to_mono(&s, 4);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 1.0).abs() < 1e-6);
        assert!((out[1] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn normalize_silence_is_unchanged() {
        let s = vec![0.0f32, 0.0, 0.0];
        assert_eq!(normalize_peak(&s, NORMALIZE_TARGET_PEAK), s);
    }

    #[test]
    fn normalize_scales_peak_to_target() {
        let out = normalize_peak(&[0.1, -0.4, 0.2], NORMALIZE_TARGET_PEAK);
        assert!((peak(&out) - NORMALIZE_TARGET_PEAK).abs() < 1e-6);
    }

    #[test]
    fn normalize_already_at_target_is_stable() {
        let s = vec![0.8f32, -0.4, 0.0];
        let out = normalize_peak(&s, NORMALIZE_TARGET_PEAK);
        for (a, b) in s.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn i16_bytes_roundtrip_known_values() {
        // 0, max positive, min negative
        let bytes = [0u8, 0, 0xFF, 0x7F, 0x00, 0x80];
        let out = i16_bytes_to_f32(&bytes);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[1] - (32767.0 / 32768.0)).abs() < 1e-6);
        assert!((out[2] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn rms_and_peak_basics() {
        assert_eq!(peak(&[]), 0.0);
        assert_eq!(rms(&[]), 0.0);
        let s = [0.5f32, -0.5, 0.5, -0.5];
        assert!((peak(&s) - 0.5).abs() < 1e-6);
        assert!((rms(&s) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn duration_ms_computes() {
        let one_sec = vec![0.0f32; 16_000];
        assert_eq!(duration_ms(&one_sec, 16_000), 1000);
        assert_eq!(duration_ms(&one_sec[..8_000], 16_000), 500);
        assert_eq!(duration_ms(&one_sec, 0), 0);
    }
}
