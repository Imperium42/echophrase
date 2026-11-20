// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! `echophrase record` - start a recording.
//!
//! Default behaviour detaches a background worker process (`echophrase worker`)
//! so the terminal is freed immediately. `--foreground` records in-process and
//! blocks until Ctrl+C.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::cli::RecordArgs;
use crate::ops;
use crate::session::{self, Session};
use crate::{fmt, recorder};

pub fn run(args: RecordArgs) -> Result<()> {
    if args.foreground {
        // Foreground recording blocks the current process until Ctrl+C, so
        // it stays out of the ops layer (the MCP `record` tool only ever
        // starts a background worker - see the tool table in cli/README.md).
        let output = args.output.clone().unwrap_or_else(ops::default_output);
        return run_foreground(output, args.device);
    }

    let started = ops::start_background_recording(args.output, args.device, args.force)?;
    println!("\u{25CF} Recording started (pid {}).", started.pid);
    println!("  Output: {}", started.output_path);
    println!("  Stop with: echophrase stop");
    Ok(())
}

/// Record in the current process until Ctrl+C or an external `echophrase stop`.
fn run_foreground(output: PathBuf, device: Option<String>) -> Result<()> {
    Session {
        pid: std::process::id(),
        output_path: output.to_string_lossy().into_owned(),
        started_at: session::now_unix(),
        foreground: true,
    }
    .save()?;

    let stop = Arc::new(AtomicBool::new(false));

    let on_signal = stop.clone();
    ctrlc::set_handler(move || on_signal.store(true, Ordering::SeqCst))
        .context("installing Ctrl+C handler")?;

    // Also honor an external `echophrase stop` (writes the stop flag).
    let on_flag = stop.clone();
    std::thread::spawn(move || {
        while !on_flag.load(Ordering::SeqCst) {
            if session::stop_requested() {
                on_flag.store(true, Ordering::SeqCst);
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    });

    println!("\u{25CF} Recording... press Ctrl+C to stop.");
    println!("  Output: {}", output.display());

    let started = session::now_unix();
    let info = recorder::capture_to_wav(&output, device.as_deref(), stop)?;
    let elapsed = session::now_unix().saturating_sub(started);

    session::clear_stop().ok();
    Session::clear().ok();

    let resampled = if info.source_rate != info.sample_rate {
        format!(" (resampled from {} Hz)", info.source_rate)
    } else {
        String::new()
    };
    println!(
        "\n\u{25A0} Saved {} ({}, {} Hz mono){}",
        output.display(),
        fmt::duration(elapsed),
        info.sample_rate,
        resampled
    );
    Ok(())
}
