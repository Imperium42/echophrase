// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Suppress WER popups (eg nvidia-smi.exe 0xC0000142) before ORT preload-dylibs ctors fire.
// .CRT$XCB runs before .CRT$XCU where the `ctor` crate places preload-dylibs constructors.
#[cfg(target_os = "windows")]
mod early_error_mode {
    use windows_sys::Win32::System::Diagnostics::Debug::{
        SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
    };

    extern "C" fn suppress_wer_dialogs() {
        // SAFETY: SetErrorMode is process-global and thread-safe.
        unsafe { SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX) };
    }

    #[used]
    #[unsafe(link_section = ".CRT$XCB")]
    static EARLY_SET_ERROR_MODE: extern "C" fn() = suppress_wer_dialogs;
}

fn main() {
    // Set PulseAudio server for WSLg if available. Capture (echo-audio) routes
    // through `parec` on WSL and inherits this env var; see echo_audio::wsl.
    if std::path::Path::new(echo_audio::wsl::WSLG_PULSE_SERVER).exists() {
        unsafe {
            std::env::set_var("PULSE_SERVER", echo_audio::wsl::WSLG_PULSE_SERVER);
        }
        println!(
            "WSLg detected - PulseAudio configured for {}",
            echo_audio::wsl::WSLG_PULSE_SERVER
        );
    }

    echophrase_lib::run();
}
