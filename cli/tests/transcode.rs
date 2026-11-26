// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! End-to-end tests for `echophrase-cli transcode`, driving the real compiled
//! binary (via `CARGO_BIN_EXE_echophrase-cli`) against synthetic WAV inputs.

use std::path::Path;
use std::process::Command;

use hound::{SampleFormat, WavSpec, WavWriter};

/// Write a stereo 48 kHz, 16-bit WAV of `frames` interleaved L/R sample pairs.
fn write_stereo_48k(path: &Path, frames: &[(i16, i16)]) {
    let spec = WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut w = WavWriter::create(path, spec).unwrap();
    for &(l, r) in frames {
        w.write_sample(l).unwrap();
        w.write_sample(r).unwrap();
    }
    w.finalize().unwrap();
}

/// A short loud tone so normalization has something to scale.
fn tone_frames() -> Vec<(i16, i16)> {
    (0..48_000)
        .map(|i| {
            let t = i as f32 / 48_000.0;
            let v = (0.3 * (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 32767.0) as i16;
            (v, v)
        })
        .collect()
}

fn run_transcode(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_echophrase-cli"))
        .arg("transcode")
        .args(args)
        .output()
        .expect("failed to run transcode")
}

#[test]
fn transcodes_stereo_48k_to_mono_16k() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    let output = dir.path().join("out.wav");
    write_stereo_48k(&input, &tone_frames());

    let out = run_transcode(&[
        input.as_os_str(),
        std::ffi::OsStr::new("-o"),
        output.as_os_str(),
    ]);
    assert!(
        out.status.success(),
        "transcode failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let reader = hound::WavReader::open(&output).expect("output WAV should exist");
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16_000);
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.bits_per_sample, 16);

    // 1s of 48 kHz audio -> ~16k samples at 16 kHz (floor of len/3).
    assert!(reader.len() > 15_000 && reader.len() <= 16_000);
}

#[test]
fn normalized_output_is_louder_than_no_normalize() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.wav");
    write_stereo_48k(&input, &tone_frames());

    let normalized = dir.path().join("norm.wav");
    let faithful = dir.path().join("faithful.wav");

    let a = run_transcode(&[
        input.as_os_str(),
        std::ffi::OsStr::new("-o"),
        normalized.as_os_str(),
    ]);
    assert!(a.status.success());

    let b = run_transcode(&[
        input.as_os_str(),
        std::ffi::OsStr::new("-o"),
        faithful.as_os_str(),
        std::ffi::OsStr::new("--no-normalize"),
    ]);
    assert!(b.status.success());

    // The source tone peaks at ~0.3; normalization lifts it toward 0.8, so the
    // normalized file's peak sample magnitude must exceed the faithful one's.
    assert!(peak(&normalized) > peak(&faithful));
}

/// Peak absolute 16-bit sample magnitude in a WAV file.
fn peak(path: &Path) -> i32 {
    hound::WavReader::open(path)
        .unwrap()
        .into_samples::<i32>()
        .map(|s| s.unwrap().abs())
        .max()
        .unwrap_or(0)
}
