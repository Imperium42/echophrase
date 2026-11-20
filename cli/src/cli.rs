// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Command-line interface definition (clap).
//!
//! Keeping the `clap` types in one module makes the surface easy to scan; the
//! `mcp` subcommand's tools map onto the same underlying `ops` functions
//! these argument shapes call into.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "echophrase",
    version,
    about = "Echophrase CLI - record your microphone from the terminal.",
    long_about = "Echophrase CLI is a small, cross-platform helper for capturing microphone \
audio from the command line.\n\n\
It records to a 16-bit mono WAV file and is designed to be driven either \
interactively or by other programs - including its own `mcp` subcommand, \
which exposes these same record/stop actions to AI agents.\n\n\
State (the active session and the output path) lives under a per-user directory. \
Override that location with the ECHOPHRASE_HOME environment variable.",
    propagate_version = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Start recording the microphone.
    ///
    /// By default this detaches a background worker and returns immediately, so
    /// you can keep using the terminal and stop later with `echophrase stop`.
    /// Use --foreground to record in the current process until Ctrl+C instead.
    Record(RecordArgs),

    /// Stop the active recording and finalize the WAV file.
    Stop(StopArgs),

    /// Show whether a recording is currently in progress.
    Status,

    /// Convert an existing WAV file to the canonical 16 kHz mono format.
    ///
    /// Downmixes to mono, resamples to 16 kHz, and (by default) peak-normalizes
    /// so the output is ready for Echophrase's transcription pipeline - the same
    /// shape the live recorder produces.
    Transcode(TranscodeArgs),

    /// Transcribe a WAV file using the running desktop app's loaded model.
    ///
    /// Requires the Echophrase desktop app to be running - it owns the
    /// loaded transcription model and this command talks to it over a
    /// localhost-only IPC connection. If the app isn't running, this exits
    /// with a clear error telling you to start it.
    Transcribe(TranscribeArgs),

    /// Run as a minimal MCP (Model Context Protocol) server over stdio.
    ///
    /// Exposes `record`, `stop`, `status`, `transcode`, and `transcribe` as
    /// MCP tools so AI agents can drive voice capture. Runs until stdin
    /// closes; see cli/README.md for a Claude Code `.mcp.json` example.
    Mcp,

    /// Internal: background capture worker. Spawned by `record`; do not run directly.
    #[command(hide = true)]
    Worker(WorkerArgs),
}

#[derive(Args, Debug)]
pub struct RecordArgs {
    /// Where to write the WAV file.
    ///
    /// Defaults to a timestamped file inside the recordings directory under
    /// ECHOPHRASE_HOME (printed when recording starts).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Input device name to capture from.
    ///
    /// Defaults to the system default input device.
    #[arg(short, long, value_name = "NAME")]
    pub device: Option<String>,

    /// Record in the foreground and block until Ctrl+C, instead of detaching a
    /// background worker.
    #[arg(short, long)]
    pub foreground: bool,

    /// Clear any stale session and start a new recording.
    ///
    /// Use this if a previous recording did not shut down cleanly and `record`
    /// reports that a session is already in progress.
    #[arg(long)]
    pub force: bool,
}

#[derive(Args, Debug)]
pub struct StopArgs {
    /// Seconds to wait for the background worker to finalize the file before
    /// returning.
    #[arg(short, long, value_name = "SECS", default_value_t = 10)]
    pub timeout: u64,
}

#[derive(Args, Debug)]
pub struct TranscodeArgs {
    /// Source WAV file to convert.
    pub input: PathBuf,

    /// Where to write the 16 kHz mono WAV.
    ///
    /// Defaults to a sibling of the input named `<input>.16k.wav`.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Skip peak normalization and preserve the original levels.
    #[arg(long)]
    pub no_normalize: bool,
    // TODO: add `--stdin`/`--stdout` streaming so the MCP server can pipe
    // audio without temp files. See cli/TODO.md.
}

#[derive(Args, Debug)]
pub struct TranscribeArgs {
    /// WAV file to transcribe.
    pub input: PathBuf,
}

#[derive(Args, Debug)]
pub struct WorkerArgs {
    /// Destination WAV path chosen by the parent `record` invocation.
    #[arg(long, value_name = "FILE")]
    pub output: PathBuf,

    /// Optional input device name forwarded from `record`.
    #[arg(long, value_name = "NAME")]
    pub device: Option<String>,
}
