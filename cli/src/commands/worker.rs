// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! `echophrase-cli worker` - the detached background capture process.
//!
//! This subcommand is hidden from `--help`; it is spawned by `record` and is
//! not meant to be run by hand. It captures audio until the stop flag appears
//! (set by `echophrase-cli stop`) or it receives a termination signal, then clears
//! the session so `stop`/`status` see an idle system.

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::cli::WorkerArgs;
use crate::recorder;
use crate::session::{self, Session};

pub fn run(args: WorkerArgs) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));

    // Best-effort signal handling in addition to the stop flag.
    let on_signal = stop.clone();
    let _ = ctrlc::set_handler(move || on_signal.store(true, Ordering::SeqCst));

    // Poll the stop flag written by `echophrase-cli stop`.
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

    let result = recorder::capture_to_wav(Path::new(&args.output), args.device.as_deref(), stop);

    // Always leave the system idle, even if capture failed.
    session::clear_stop().ok();
    Session::clear().ok();

    result.map(|_| ())
}
