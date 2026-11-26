// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! `echophrase-cli transcribe` - send a WAV file to the running desktop app's
//! loaded transcription model and print the transcript.
//!
//! Unlike `record`/`stop`/`transcode`, this command has nothing to do
//! without the desktop app: the model lives there, not in this small
//! terminal helper. See `ops::transcribe` for the IPC call and its friendly
//! "app must be running" error.

use anyhow::Result;

use crate::cli::TranscribeArgs;
use crate::ops;

pub fn run(args: TranscribeArgs) -> Result<()> {
    let result = ops::transcribe(&args.input)?;
    println!("{}", result.text);
    Ok(())
}
