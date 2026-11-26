// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! `echophrase-cli stop` - end the active recording and finalize the WAV.

use anyhow::Result;

use crate::cli::StopArgs;
use crate::fmt;
use crate::ops;

pub fn run(args: StopArgs) -> Result<()> {
    let result = ops::stop(args.timeout)?;

    // A foreground recorder owns its own terminal; it will finalize and print
    // its own summary. We just deliver the stop request.
    if result.foreground {
        println!(
            "Stop requested for the foreground recording (pid {}).",
            result.pid
        );
        return Ok(());
    }

    if result.timed_out {
        println!(
            "Stop requested, but the worker did not confirm within {}s.",
            args.timeout
        );
        println!("File (may still be finalizing): {}", result.wav_path);
        return Ok(());
    }

    println!(
        "\u{25A0} Saved {} ({}, {})",
        result.wav_path,
        fmt::duration(result.duration_secs.unwrap_or(0)),
        fmt::bytes(result.size_bytes.unwrap_or(0))
    );
    Ok(())
}
