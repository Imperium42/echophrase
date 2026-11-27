// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Voice-band level meter for the recording overlay waveform.
//!
//! Turns the 16 kHz mono capture stream into `BAND_COUNT` normalized (0-1)
//! band levels, one frame per `FFT_SIZE` samples (32 ms, ~31 fps).
//!
//! Approach credit: the FFT-bands-in-Rust idea (Hann window, voice-range
//! buckets, dB normalization, power curve) is borrowed from Handy
//! (<https://github.com/cjpais/Handy>, MIT License, Copyright (c) CJ Pais).
//! The band layout, calibration and adaptive noise floor here are our own.

use std::sync::Arc;

use rustfft::{Fft, FftPlanner, num_complex::Complex};

/// Capture sample rate (echo-audio always delivers 16 kHz mono).
const SAMPLE_RATE: f32 = 16_000.0;
/// 512 samples = 32 ms per frame at 16 kHz, 31.25 Hz per bin.
pub const FFT_SIZE: usize = 512;
/// Number of bands sent to the overlay (the frontend mirrors them).
pub const BAND_COUNT: usize = 12;
/// Voice-intelligibility range the bands cover, log-spaced.
const MIN_HZ: f32 = 250.0;
const MAX_HZ: f32 = 4_000.0;
/// dBFS window mapped onto 0..1 after noise-floor subtraction.
const DB_RANGE: f32 = 42.0;
/// Absolute floor: anything quieter than this is silence.
const DB_ABS_FLOOR: f32 = -72.0;
/// Perceptual curve so quiet speech still moves the bars.
const CURVE: f32 = 0.65;
/// Noise floor rises slowly (room hum) and falls fast (a gap in the noise).
const FLOOR_RISE: f32 = 0.004;
/// Never let the adaptive floor swallow normal speech.
const FLOOR_CEILING: f32 = -40.0;

pub struct LevelMeter {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    pending: Vec<f32>,
    scratch: Vec<Complex<f32>>,
    /// Inclusive-exclusive FFT bin range per band.
    band_bins: Vec<(usize, usize)>,
    noise_floor: [f32; BAND_COUNT],
}

impl LevelMeter {
    pub fn new() -> Self {
        let fft = FftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
        // Periodic Hann window.
        let window = (0..FFT_SIZE)
            .map(|i| {
                let phase = 2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();
        Self {
            fft,
            window,
            pending: Vec::with_capacity(FFT_SIZE * 2),
            scratch: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            band_bins: band_bins(),
            noise_floor: [DB_ABS_FLOOR; BAND_COUNT],
        }
    }

    /// Feed capture samples; returns the newest completed frame, if any.
    /// Older frames from a large chunk are dropped (the UI only wants "now").
    pub fn push(&mut self, samples: &[f32]) -> Option<[f32; BAND_COUNT]> {
        self.pending.extend_from_slice(samples);
        let mut latest = None;
        while self.pending.len() >= FFT_SIZE {
            latest = Some(self.analyze_front());
            self.pending.drain(..FFT_SIZE);
        }
        latest
    }

    fn analyze_front(&mut self) -> [f32; BAND_COUNT] {
        for (slot, (s, w)) in self
            .scratch
            .iter_mut()
            .zip(self.pending.iter().zip(self.window.iter()))
        {
            *slot = Complex::new(s * w, 0.0);
        }
        self.fft.process(&mut self.scratch);

        // A full-scale sine through a Hann window peaks at N/4: normalize so
        // that reads as 0 dBFS.
        let norm = (FFT_SIZE as f32 / 4.0).powi(2);
        let db: [f32; BAND_COUNT] = std::array::from_fn(|band| {
            let (lo, hi) = self.band_bins[band];
            let peak = self.scratch[lo..hi]
                .iter()
                .map(|c| c.norm_sqr() / norm)
                .fold(0.0_f32, f32::max);
            10.0 * (peak + 1e-12).log10()
        });
        std::array::from_fn(|band| self.normalize(band, db[band]))
    }

    fn normalize(&mut self, band: usize, db: f32) -> f32 {
        let floor = &mut self.noise_floor[band];
        *floor = if db < *floor {
            db.max(DB_ABS_FLOOR)
        } else {
            (*floor + (db - *floor) * FLOOR_RISE).min(FLOOR_CEILING)
        };
        let above = (db - *floor - 3.0).max(0.0);
        (above / DB_RANGE).clamp(0.0, 1.0).powf(CURVE)
    }
}

/// Log-spaced band edges in FFT bins, each band at least one bin wide.
fn band_bins() -> Vec<(usize, usize)> {
    let hz_per_bin = SAMPLE_RATE / FFT_SIZE as f32;
    let ratio = (MAX_HZ / MIN_HZ).powf(1.0 / BAND_COUNT as f32);
    (0..BAND_COUNT)
        .map(|b| {
            let lo_hz = MIN_HZ * ratio.powi(b as i32);
            let hi_hz = lo_hz * ratio;
            let lo = (lo_hz / hz_per_bin).floor() as usize;
            let hi = ((hi_hz / hz_per_bin).ceil() as usize).max(lo + 1);
            (lo, hi.min(FFT_SIZE / 2))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, amp: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| amp * (2.0 * std::f32::consts::PI * hz * i as f32 / SAMPLE_RATE).sin())
            .collect()
    }

    #[test]
    fn bands_are_ordered_and_in_range() {
        let bins = band_bins();
        assert_eq!(bins.len(), BAND_COUNT);
        for w in bins.windows(2) {
            assert!(w[0].0 <= w[1].0);
        }
        assert!(bins.iter().all(|&(lo, hi)| lo < hi && hi <= FFT_SIZE / 2));
    }

    #[test]
    fn silence_reads_zero() {
        let mut meter = LevelMeter::new();
        let frame = meter.push(&vec![0.0; FFT_SIZE]).unwrap();
        assert!(frame.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn needs_a_full_frame() {
        let mut meter = LevelMeter::new();
        assert!(meter.push(&vec![0.1; FFT_SIZE - 1]).is_none());
        assert!(meter.push(&[0.1]).is_some());
    }

    #[test]
    fn tone_lights_its_own_band_most() {
        let mut meter = LevelMeter::new();
        let frame = meter.push(&sine(1_000.0, 0.3, FFT_SIZE)).unwrap();
        let loudest = frame
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap();
        let (lo, hi) = band_bins()[loudest];
        let bin_1k = (1_000.0 / (SAMPLE_RATE / FFT_SIZE as f32)) as usize;
        assert!((lo..=hi).contains(&bin_1k), "band {loudest} = {lo}..{hi}");
        assert!(frame[loudest] > 0.5);
    }

    #[test]
    fn large_chunk_returns_only_latest_frame() {
        let mut meter = LevelMeter::new();
        let mut audio = sine(1_000.0, 0.3, FFT_SIZE);
        audio.extend(vec![0.0; FFT_SIZE]);
        let frame = meter.push(&audio).unwrap();
        assert!(frame.iter().all(|&v| v == 0.0));
    }
}
