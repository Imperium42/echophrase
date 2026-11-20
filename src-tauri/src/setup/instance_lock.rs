// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use std::sync::OnceLock;

// File-lock-based single-instance fallback. tauri-plugin-single-instance uses
// D-Bus on Linux, which isn't running in WSL - so without this we can get
// duplicate processes, orphaned global shortcuts, and zombie windows.
//
// The File is parked in a static so its lock lives for the entire process
// lifetime. OS releases the lock when the process dies (even via _exit).
static INSTANCE_LOCK: OnceLock<std::fs::File> = OnceLock::new();

/// Acquire a process-wide exclusive file lock to prevent duplicate instances.
/// Returns true on success (we're the first/only instance). Returns false if
/// another live instance already holds the lock.
///
/// Lock path: `~/.local/share/com.imperium42.echophrase/instance.lock`
/// (or the platform equivalent - uses `dirs::data_local_dir`).
pub fn acquire_instance_lock() -> bool {
    // fs4::FileExt provides the `try_lock()` method we use below. Rust 1.89 added
    // `File::try_lock` to std with a different signature (io::Result<bool>), so
    // rustc's "unused import" warning is a false positive - we NEED the trait
    // import for the `Ok(())` match arm to typecheck against fs4's API.
    #[allow(unused_imports)]
    use fs4::FileExt;
    use std::io::{Seek, SeekFrom, Write};

    let Some(base) = dirs::data_local_dir() else {
        log::warn!("[single-instance] data_local_dir unavailable - skipping file lock");
        return true; // Fail open - don't block the user if we can't find a home dir.
    };
    let dir = base.join("com.imperium42.echophrase");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!(
            "[single-instance] create_dir_all failed ({}) - skipping file lock",
            e
        );
        return true;
    }
    let path = dir.join("instance.lock");
    let mut file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            log::warn!("[single-instance] open lock file failed ({}) - skipping", e);
            return true;
        }
    };
    // fs4 1.0: try_lock() is exclusive by default, returns Result<(), TryLockError>
    // The clippy::incompatible_msrv lint fires here because Rust 1.89 added
    // File::try_lock to std - but we are calling fs4::FileExt::try_lock which
    // is stable on our MSRV. The #[allow(unused_imports)] on the use above is
    // required for the same reason.
    #[allow(clippy::incompatible_msrv)]
    match file.try_lock() {
        Ok(()) => {
            // Got the lock. Overwrite file contents with our PID so a duplicate
            // launch can show a useful message.
            let _ = file.set_len(0);
            let _ = file.seek(SeekFrom::Start(0));
            let _ = writeln!(file, "{}", std::process::id());
            log::info!("[single-instance] file lock acquired at {}", path.display());
            let _ = INSTANCE_LOCK.set(file);
            true
        }
        Err(_) => {
            // Another process holds the lock (WouldBlock) or the lock op failed.
            let prior = std::fs::read_to_string(&path).unwrap_or_default();
            let prior = prior.trim();
            eprintln!(
                "Another Echophrase instance is already running{}.\n\
                 Lock: {}\n\
                 Close the other instance or run: kill {} && rm {}",
                if prior.is_empty() {
                    String::new()
                } else {
                    format!(" (PID {})", prior)
                },
                path.display(),
                if prior.is_empty() { "<PID>" } else { prior },
                path.display()
            );
            false
        }
    }
}
