// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

// Note on structured logging:
// tracing macros (info!, warn!, etc.) automatically bridge to the log crate
// when no tracing subscriber is set. tauri_plugin_log handles all output.
// Structured fields (e.g. gpu.provider = "CoreML") are formatted into log messages.

/// Validate environment configuration and critical dependencies.
///
/// Checks:
/// - ONNX Runtime library path exists on disk
/// - Platform-specific GPU provider hints
///
/// Note: As of ort 2.0.0-rc.12, we use `load-dynamic` to load onnxruntime
/// at runtime via the C API. ORT_DYLIB_PATH must point to the ORT 1.24.2
/// shared library. The only remaining guard is the macOS 13.3+ check
/// (ORT 1.24.x requires C++20 std::to_chars in libc++).
pub fn validate_environment() -> anyhow::Result<()> {
    // macOS 13.3+ guard: ORT 1.24.x uses C++20 std::to_chars(double) which is
    // only present in libc++ on macOS 13.3 and later. On older systems we
    // disable ORT entirely so the app can fall back to non-ORT backends
    // instead of hanging at first session creation.
    #[cfg(target_os = "macos")]
    {
        if !crate::transcription::execution_provider::is_macos_13_3_or_later() {
            eprintln!(
                "[env] macOS < 13.3 detected - disabling ONNX Runtime (requires C++20 std::to_chars)"
            );
            crate::ONNX_RUNTIME_AVAILABLE.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    // Log platform-specific GPU info
    #[cfg(target_os = "macos")]
    eprintln!("[env] Platform: macOS - CoreML (Metal GPU) will be used as default provider");
    #[cfg(target_os = "windows")]
    eprintln!("[env] Platform: Windows - DirectML will be used as default provider");
    #[cfg(target_os = "linux")]
    eprintln!(
        "[env] Platform: Linux - ort with_auto_device picks the best EP at session creation time"
    );

    Ok(())
}
