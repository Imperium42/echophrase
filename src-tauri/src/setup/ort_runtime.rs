// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! ORT runtime selection and provider preloading.
//!
//! Pick which ORT runtime to load and write it to `ORT_DYLIB_PATH` before any
//! ORT API is touched.
//!
//! Precedence (Windows):
//! 1. If a dev `ORT_DYLIB_PATH` is already set (eg by `bun run tauri:dev`
//!    via `env.yaml`), honor it unchanged - local dev keeps using whatever the
//!    developer staged on their machine.
//! 2. Else, use the bundled DirectML runtime at
//!    `<exe_dir>/resources/ort/directml/onnxruntime.dll`.
//!
//! macOS uses the bundled CoreML build in `Contents/Frameworks`; Linux gets its
//! runtime from CI or the dev's `ensure-ort.mjs` script (CPU only).
//!
//! With ort's `load-dynamic` feature, onnxruntime.dll later resolves provider DLLs
//! by bare name (eg `onnxruntime_providers_shared.dll`, `DirectML.dll`) using Windows'
//! standard DLL search order. If Windows picks up the wrong version from PATH or
//! System32 first, we get STATUS_ACCESS_VIOLATION on EP registration due to a
//! version-mismatched vtable.
//!
//! `preload_ort_providers` explicitly `LoadLibrary`'s the DLLs from absolute paths
//! BEFORE any ORT API is called, so Windows' loader uses those handles when
//! onnxruntime.dll later resolves the provider by bare name.
//! See pykeio/ort#361 and ORT GetRuntimePath.

/// Captures the runtime-selection diagnostic so it can be re-emitted via
/// `log::info!` after Tauri's log plugin initialises. `select_ort_runtime`
/// runs BEFORE the log plugin is wired up, so its only contemporaneous output
/// channel is stderr - which is NUL on Windows GUI builds (no attached console).
/// `lib.rs::setup()` reads this once the logger is up and writes it to
/// `Echophrase.log` so users / support can see which DLL was loaded.
#[cfg(target_os = "windows")]
pub static RUNTIME_SELECTION_DIAGNOSTIC: std::sync::OnceLock<String> = std::sync::OnceLock::new();

#[cfg(target_os = "windows")]
fn record_diag(msg: String) {
    eprintln!("{}", msg);
    let _ = RUNTIME_SELECTION_DIAGNOSTIC.set(msg);
}

/// Pick which ORT runtime to load (dev override, else the bundled runtime).
#[cfg(target_os = "windows")]
pub fn select_ort_runtime() {
    // Read the user's saved EP preference (logged for diagnostics only).
    let preferred = crate::settings::AppSettings::load()
        .ok()
        .and_then(|s| s.execution_provider)
        .and_then(|s| crate::transcription::execution_provider::parse_execution_provider(&s));
    // Step 1: honor an explicitly-set dev path (env.yaml).
    if let Some(dev_path) = std::env::var_os("ORT_DYLIB_PATH") {
        record_diag(format!(
            "[ort] Active runtime: {} (dev ORT_DYLIB_PATH, requested EP: {:?})",
            std::path::Path::new(&dev_path).display(),
            preferred
        ));
        return;
    }

    // Step 2: bundled DirectML fallback next to exe.
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::PathBuf::from))
    else {
        record_diag("[ort] could not resolve exe dir; leaving ORT_DYLIB_PATH unset".to_string());
        return;
    };
    let rel = std::path::Path::new("resources")
        .join("ort")
        .join("directml")
        .join("onnxruntime.dll");
    let next_to_exe = exe_dir.join(&rel);
    // Dev builds: scripts/ensure-ort.mjs stages into src-tauri/resources/, but tauri-build
    // only re-copies resources to target/ when a file it already tracked changes, so a
    // freshly staged DLL can be missing next to the dev exe. Without this fallback ORT
    // loads the bare name and picks up Windows' old inbox System32\onnxruntime.dll.
    #[cfg(debug_assertions)]
    let bundled = if next_to_exe.is_file() {
        next_to_exe
    } else {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(&rel)
    };
    #[cfg(not(debug_assertions))]
    let bundled = next_to_exe;
    if !bundled.is_file() {
        record_diag(format!(
            "[ort] bundled DirectML runtime missing: {} - leaving ORT_DYLIB_PATH unset",
            bundled.display()
        ));
        return;
    }
    unsafe {
        std::env::set_var("ORT_DYLIB_PATH", &bundled);
    }
    record_diag(format!(
        "[ort] Active runtime: {} (bundled DirectML, requested EP: {:?})",
        bundled.display(),
        preferred
    ));
}

#[cfg(target_os = "macos")]
pub fn select_ort_runtime() {
    // If already set (e.g. dev env.yaml wrote an absolute path), honor it.
    if std::env::var_os("ORT_DYLIB_PATH").is_some() {
        eprintln!("[ort] macOS: honoring existing ORT_DYLIB_PATH from env");
        return;
    }

    // Production .app bundle: libonnxruntime.dylib lives at
    //   <exe>/../../Frameworks/libonnxruntime.dylib
    // i.e., Contents/MacOS/<exe> -> Contents/Frameworks/libonnxruntime.dylib.
    // We resolve via current_exe() rather than relying on @executable_path so
    // the path is absolute and works regardless of dlopen search order.
    let bundled = std::env::current_exe().ok().and_then(|exe| {
        // exe = .../Echophrase.app/Contents/MacOS/Echophrase
        // walk up to Contents/ then into Frameworks/
        exe.parent() // Contents/MacOS
            .and_then(|p| p.parent()) // Contents
            .map(|contents| contents.join("Frameworks").join("libonnxruntime.dylib"))
    });

    match bundled {
        Some(ref path) if path.is_file() => {
            eprintln!("[ort] macOS: using bundled dylib: {}", path.display());
            // SAFETY: runs before tokio/tauri spawn any threads.
            unsafe {
                std::env::set_var("ORT_DYLIB_PATH", path);
            }
        }
        Some(ref path) => {
            eprintln!(
                "[ort] macOS: bundled dylib not found at {} - ORT_DYLIB_PATH unset",
                path.display()
            );
        }
        None => {
            eprintln!("[ort] macOS: could not resolve exe path - ORT_DYLIB_PATH unset");
        }
    }
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub fn select_ort_runtime() {
    // Linux is CPU only; env.yaml ORT_DYLIB_PATH is set
    // by CI or the dev's ensure-ort.mjs script. Nothing to select dynamically.
}

/// Preload ORT execution provider DLLs from the same directory as onnxruntime.dll.
pub fn preload_ort_providers() {
    // Use eprintln! because tauri_plugin_log isn't initialized yet this early in run().
    let Some(ort_dir) = std::env::var("ORT_DYLIB_PATH")
        .ok()
        .map(std::path::PathBuf::from)
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    else {
        eprintln!("[ORT preload] ORT_DYLIB_PATH not set; skipping provider DLL preload");
        return;
    };

    // Provider DLLs that MUST live next to onnxruntime.dll.
    #[cfg(target_os = "windows")]
    let next_to_ort: &[&str] = &["onnxruntime_providers_shared.dll"];
    #[cfg(target_os = "linux")]
    let next_to_ort: &[&str] = &["libonnxruntime_providers_shared.so"];
    #[cfg(target_os = "macos")]
    let next_to_ort: &[&str] = &["libonnxruntime_providers_shared.dylib"];

    for name in next_to_ort {
        let path = ort_dir.join(name);
        if !path.exists() {
            // Optional (not every runtime ships every provider) - don't warn.
            continue;
        }
        match ort::util::preload_dylib(&path) {
            Ok(()) => eprintln!("[ORT preload] loaded {}", path.display()),
            Err(e) => eprintln!("[ORT preload] failed for {}: {}", path.display(), e),
        }
    }

    // Prefer the DirectML.dll we bundle next to onnxruntime.dll (from the
    // microsoft.ai.directml NuGet, staged by CI). Bare-name LoadLibrary would
    // hit System32, where Win10 22H2 / early Win11 can ship an older
    // DirectML that mismatches ORT 1.24's expected API. Bundling pins the
    // pair and also avoids a Defender first-launch scan on the system DLL.
    #[cfg(target_os = "windows")]
    {
        let bundled_dml = ort_dir.join("DirectML.dll");
        let preload_target: std::path::PathBuf = if bundled_dml.is_file() {
            bundled_dml
        } else {
            std::path::PathBuf::from("DirectML.dll")
        };
        match ort::util::preload_dylib(&preload_target) {
            Ok(()) => eprintln!("[ORT preload] loaded {}", preload_target.display()),
            Err(e) => eprintln!(
                "[ORT preload] DirectML.dll preload failed ({}): {}",
                preload_target.display(),
                e
            ),
        }
    }
}
