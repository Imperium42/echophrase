// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! `echophrase status` - report whether a recording is in progress.

use anyhow::Result;

use crate::fmt;
use crate::ops;
use crate::session;

pub fn run() -> Result<()> {
    let result = ops::status()?;
    match result.session {
        Some(s) => {
            let elapsed = session::now_unix().saturating_sub(s.started_at);
            println!("\u{25CF} Recording in progress");
            println!("  pid:     {}", s.pid);
            println!(
                "  mode:    {}",
                if s.foreground {
                    "foreground"
                } else {
                    "background"
                }
            );
            println!("  elapsed: {}", fmt::duration(elapsed));
            println!("  output:  {}", s.output_path);
        }
        None => println!("\u{25CB} Idle (no active recording)."),
    }
    Ok(())
}
