// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Echophrase CLI - a small cross-platform microphone recorder for the terminal.
//!
//! Subcommands: `record`, `stop`, `status`, `transcode`, `mcp` (and the hidden
//! `worker`). The recorder and session modules are decoupled from the CLI so
//! the `mcp` subcommand (see `commands/mcp.rs`) can reuse them directly
//! through the `ops` layer.

mod cli;
mod fmt;
mod ops;
mod paths;
mod recorder;
mod session;

mod commands {
    pub mod mcp;
    pub mod record;
    pub mod status;
    pub mod stop;
    pub mod transcode;
    pub mod transcribe;
    pub mod worker;
}

use clap::Parser;

use cli::{Cli, Command};

fn main() {
    let code = match run() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("error: {err:#}");
            1
        }
    };
    std::process::exit(code);
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Record(args) => commands::record::run(args),
        Command::Stop(args) => commands::stop::run(args),
        Command::Status => commands::status::run(),
        Command::Transcode(args) => commands::transcode::run(args),
        Command::Transcribe(args) => commands::transcribe::run(args),
        Command::Mcp => commands::mcp::run(),
        Command::Worker(args) => commands::worker::run(args),
    }
}
