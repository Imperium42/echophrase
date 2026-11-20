// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Linear-interpolation resampling: a batch function for whole clips and a
//! stateful [`StreamingResampler`] for live capture, guaranteed to produce
//! identical output for identical input regardless of how it is chunked.
//!
//! Linear interpolation is simple but effective for speech and the common
//! ratios (48 kHz -> 16 kHz is exactly 3:1).
// TODO: consider a sinc/`rubato`-based resampler for higher-fidelity
// downsampling. See cli/TODO.md.

/// Resample a whole clip from `from_rate` to `to_rate`.
///
/// Same-rate or empty input passes through as a copy. Output length is
/// `floor(len * to_rate / from_rate)`.
pub fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = ((samples.len() as f64) / ratio).floor() as usize;
    let mut resampled = Vec::with_capacity(new_len);
    for i in 0..new_len {
        let src_pos = i as f64 * ratio;
        resampled.push(interpolate_at(samples, src_pos));
    }
    resampled
}

/// Linearly interpolate `samples` at fractional position `src_pos`, clamping to
/// the final sample at the right edge.
fn interpolate_at(samples: &[f32], src_pos: f64) -> f32 {
    let src_idx = src_pos.floor() as usize;
    let frac = src_pos - src_idx as f64;
    if src_idx + 1 < samples.len() {
        let s0 = samples[src_idx] as f64;
        let s1 = samples[src_idx + 1] as f64;
        (s0 + frac * (s1 - s0)) as f32
    } else {
        samples[samples.len() - 1]
    }
}

/// Stateful streaming resampler for live capture.
///
/// Devices deliver audio in arbitrary chunk sizes at their native rate
/// (44.1/48 kHz); the pipeline wants a continuous 16 kHz stream. This carries
/// the fractional source position and one sample of history across calls so
/// chunk boundaries interpolate exactly like [`resample_linear`] over the
/// concatenated input (verified by the equivalence tests below).
pub struct StreamingResampler {
    from_rate: u32,
    to_rate: u32,
    /// from/to ratio cached as f64.
    ratio: f64,
    /// Next output sample's position in the GLOBAL source stream.
    next_src_pos: f64,
    /// How many source samples have been consumed before the current chunk.
    src_consumed: u64,
    /// Last sample of the previous chunk, for interpolation across the seam.
    carry: Option<f32>,
}

impl StreamingResampler {
    /// Create a resampler from `from_rate` to `to_rate`.
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Self {
            from_rate,
            to_rate,
            ratio: from_rate as f64 / to_rate as f64,
            next_src_pos: 0.0,
            src_consumed: 0,
            carry: None,
        }
    }

    /// Consume `input` (at `from_rate`), appending resampled output (at
    /// `to_rate`) to `out`. Passthrough copy when the rates match.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        if self.from_rate == self.to_rate {
            out.extend_from_slice(input);
            return;
        }
        if input.is_empty() {
            return;
        }

        // Emit every output sample whose interpolation window [idx, idx+1]
        // is fully available given this chunk (plus the carried sample).
        loop {
            let rel = self.next_src_pos - self.src_consumed as f64;
            let value = match self.sample_window(input, rel) {
                Some(v) => v,
                None => break, // needs data from the next chunk
            };
            out.push(value);
            self.next_src_pos += self.ratio;
        }

        self.carry = Some(input[input.len() - 1]);
        self.src_consumed += input.len() as u64;
    }

    /// Interpolate at position `rel` relative to the current chunk, where
    /// `rel == -1.0..0.0` falls inside the carried seam from the previous
    /// chunk. Returns `None` when the window extends past this chunk's end.
    fn sample_window(&self, input: &[f32], rel: f64) -> Option<f32> {
        let idx = rel.floor() as i64;
        let frac = rel - idx as f64;

        // Left sample of the interpolation window.
        let s0 = if idx < 0 {
            self.carry? // position is in the seam; needs the carried sample
        } else if (idx as usize) < input.len() {
            input[idx as usize]
        } else {
            return None;
        };

        // Right sample of the window. At the exact final position (frac == 0)
        // no right neighbor is needed.
        if frac == 0.0 {
            return Some(s0);
        }
        let right = idx + 1;
        if right >= 0 && (right as usize) < input.len() {
            let s1 = input[right as usize];
            Some((s0 as f64 + frac * (s1 as f64 - s0 as f64)) as f32)
        } else {
            None
        }
    }

    /// Reset all streaming state (e.g. when a capture session restarts).
    pub fn reset(&mut self) {
        self.next_src_pos = 0.0;
        self.src_consumed = 0;
        self.carry = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_same_rate_is_passthrough() {
        let s = vec![0.1, 0.2, -0.3, 0.4];
        assert_eq!(resample_linear(&s, 16_000, 16_000), s);
    }

    #[test]
    fn batch_empty_is_empty() {
        assert!(resample_linear(&[], 48_000, 16_000).is_empty());
    }

    #[test]
    fn batch_downsample_48k_to_16k_thirds_the_length() {
        let s: Vec<f32> = (0..9).map(|i| i as f32).collect();
        let out = resample_linear(&s, 48_000, 16_000);
        assert_eq!(out.len(), 3);
        assert!((out[0] - 0.0).abs() < 1e-6);
        assert!((out[1] - 3.0).abs() < 1e-6);
        assert!((out[2] - 6.0).abs() < 1e-6);
    }

    #[test]
    fn batch_interpolates_between_samples() {
        let s = vec![0.0f32, 1.0, 2.0, 3.0];
        let out = resample_linear(&s, 16_000, 24_000);
        assert_eq!(out.len(), 6);
        assert!((out[1] - 0.6667).abs() < 1e-3);
    }

    /// The core streaming guarantee: chunked output == batch output for the
    /// same total input, no matter where the chunk boundaries fall.
    fn assert_streaming_matches_batch(from: u32, to: u32, chunk_sizes: &[usize]) {
        // A deterministic pseudo-signal long enough to cross many boundaries.
        let total: usize = chunk_sizes.iter().sum();
        let signal: Vec<f32> = (0..total)
            .map(|i| ((i as f32 * 0.13).sin() + (i as f32 * 0.031).cos()) * 0.4)
            .collect();

        let batch = resample_linear(&signal, from, to);

        let mut streamed = Vec::new();
        let mut r = StreamingResampler::new(from, to);
        let mut offset = 0;
        for &n in chunk_sizes {
            r.process(&signal[offset..offset + n], &mut streamed);
            offset += n;
        }

        // The stream may lag batch by at most the samples whose interpolation
        // window was not complete at the final chunk edge.
        assert!(
            batch.len() - streamed.len() <= 2,
            "len {} vs {}",
            streamed.len(),
            batch.len()
        );
        for (i, (a, b)) in batch.iter().zip(streamed.iter()).enumerate() {
            assert!((a - b).abs() < 1e-6, "mismatch at {i}: {a} vs {b}");
        }
    }

    #[test]
    fn streaming_matches_batch_48k_even_chunks() {
        assert_streaming_matches_batch(48_000, 16_000, &[480; 20]);
    }

    #[test]
    fn streaming_matches_batch_48k_ragged_chunks() {
        assert_streaming_matches_batch(48_000, 16_000, &[1, 7, 256, 3, 999, 480, 2, 64, 1024, 5]);
    }

    #[test]
    fn streaming_matches_batch_44_1k_ragged_chunks() {
        assert_streaming_matches_batch(44_100, 16_000, &[441, 10, 333, 1024, 7, 900, 441]);
    }

    #[test]
    fn streaming_passthrough_when_rates_match() {
        let mut out = Vec::new();
        let mut r = StreamingResampler::new(16_000, 16_000);
        r.process(&[0.1, 0.2], &mut out);
        r.process(&[0.3], &mut out);
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn streaming_reset_restarts_cleanly() {
        let signal: Vec<f32> = (0..96).map(|i| i as f32 * 0.01).collect();
        let mut r = StreamingResampler::new(48_000, 16_000);
        let mut first = Vec::new();
        r.process(&signal, &mut first);
        r.reset();
        let mut second = Vec::new();
        r.process(&signal, &mut second);
        assert_eq!(first, second);
    }
}
