// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! `echophrase-cli transcode` - convert an existing WAV file to the canonical
//! 16 kHz mono format the transcription pipeline expects.
//!
//! This is the file-ingest counterpart to `record`: instead of capturing the
//! microphone, it reads a WAV from disk, downmixes to mono, resamples to 16 kHz,
//! and (by default) peak-normalizes - producing the same shape a live recording
//! would, so the output drops straight into the desktop app or the MCP server.

use anyhow::Result;

use crate::cli::TranscodeArgs;
use crate::fmt;
use crate::ops;

// TODO: single-file only. Broaden to batch / glob / directory input and a
// `--recursive` mode. See cli/TODO.md.
pub fn run(args: TranscodeArgs) -> Result<()> {
    let result = ops::transcode(&args.input, args.output, !args.no_normalize)?;

    let resampled_note = if result.source_rate != result.sample_rate {
        format!(" (resampled from {} Hz)", result.source_rate)
    } else {
        String::new()
    };
    let norm_note = if result.normalized {
        ""
    } else {
        " [levels preserved]"
    };
    println!(
        "\u{25A0} Wrote {} ({}, {}, {} Hz mono){}{}",
        result.output_path,
        fmt::duration(result.duration_secs),
        fmt::bytes(result.size_bytes),
        result.sample_rate,
        resampled_note,
        norm_note
    );
    Ok(())
}
