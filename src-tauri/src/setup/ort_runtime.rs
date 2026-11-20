// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! ORT runtime selection and provider preloading.
//!
//! Pick which ORT runtime to load and write it to `ORT_DYLIB_PATH` before any
//! ORT API is touched.
//!
//! Precedence (Windows):
//! 1. If the user explicitly picked CUDA in Settings AND the CUDA plugin is
//!    installed AND the machine looks like a working NVIDIA setup (nvidia-smi
//!    OK + CUDA_PATH + CUDNN_PATH), point ORT at the plugin's onnxruntime.dll
//!    (CUDA EP). Mere presence of the plugin is NOT enough - the auto-pick
//!    heuristic was a silent footgun: users who selected DirectML in Settings
//!    would end up with the CUDA-only ORT DLL loaded process-wide, which lacks
//!    the DirectML EP and causes session creation to hang on external-data models.
//! 2. Else, if a dev `ORT_DYLIB_PATH` is already set (eg by `bun run tauri:dev`
//!    via `env.yaml`), honor it unchanged - local dev keeps using whatever the
//!    developer staged on their machine.
//! 3. Else, fall back to the bundled DirectML runtime at
//!    `<exe_dir>/resources/ort/directml/onnxruntime.dll`.
//!
//! Windows-only: on Linux the installer already bundles the CUDA ORT build, on
//! macOS the CoreML build - there is nothing to select dynamically.
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
//!
//! We preload `onnxruntime_providers_cuda.dll` only when it actually exists in
//! the ORT dir (ie the user is on the CUDA runtime). For the DirectML runtime
//! that file is absent and we skip it.
//!
//! CUDA transitive deps (cuDNN, cudart, cublas, ...) live in directories outside
//! the ORT plugin dir. Windows `LoadLibraryExW` with `LOAD_WITH_ALTERED_SEARCH_PATH`
//! (what the ort crate uses) ignores process `PATH` for transitive resolution, so
//! we preload those deps explicitly from absolute paths BEFORE loading the CUDA
//! EP. Once a DLL is open by name, Windows satisfies subsequent bare-name lookups
//! from the existing handle.
//! Resolve CUDA runtime DLLs (cudart/cublas/cusparse/curand/cusolver/cufft) from
//! `%CUDA_PATH%`. CUDA 13 moved them to `bin/x64/`; CUDA 12 keeps them flat in
//! `bin/`. Recursive glob handles both layouts.

/// Pick which ORT runtime (DirectML bundled vs CUDA plugin). Runs AFTER the
/// two preload steps so it can test-load the plugin's CUDA provider to
/// confirm the plugin actually works on this machine (catches CUDA version
/// mismatch) before committing to it. If the test-load fails, falls back to
/// the bundled DirectML runtime so the app still has GPU acceleration.
///
/// Windows-only: on Linux the installer already bundles the CUDA ORT build, on
/// macOS the CoreML build - there is nothing to select dynamically.
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

#[cfg(target_os = "windows")]
pub fn select_ort_runtime() {
    // Read the user's saved EP preference. CUDA plugin is ONLY allowed when the
    // user explicitly selected it - the old "auto-pick if installed" heuristic
    // silently overrode DirectML users (CUDA-only DLL lacks the DirectML EP).
    let preferred = crate::settings::AppSettings::load()
        .ok()
        .and_then(|s| s.execution_provider)
        .and_then(|s| crate::transcription::execution_provider::parse_execution_provider(&s));
    // Step 1 (CUDA only): plugin runtime, ONLY when the user explicitly picked CUDA.
    #[cfg(feature = "cuda")]
    let user_requested_cuda = matches!(
        preferred,
        Some(crate::transcription::execution_provider::ExecutionProviderType::Cuda)
    );
    #[cfg(feature = "cuda")]
    if user_requested_cuda {
        let plugin_dll = dirs::data_local_dir().map(|d| {
            d.join("com.imperium42.echophrase")
                .join("ort")
                .join("cuda")
                .join("onnxruntime.dll")
        });
        let plugin_present = plugin_dll.as_ref().is_some_and(|p| p.is_file())
            && crate::transcription::execution_provider::nvidia_smi_ok()
            && std::env::var("CUDA_PATH").is_ok()
            && std::env::var("CUDNN_PATH").is_ok();

        if plugin_present {
            let p = plugin_dll.as_ref().unwrap();
            let provider = p
                .parent()
                .map(|d| d.join("onnxruntime_providers_cuda.dll"))
                .unwrap_or_else(|| p.clone());
            if crate::ort_plugin::can_plugin_load(&provider) {
                unsafe {
                    std::env::set_var("ORT_DYLIB_PATH", p);
                }
                record_diag(format!(
                    "[ort] Active runtime: {} (user requested CUDA)",
                    p.display()
                ));
                return;
            }
            record_diag(
                "[ort] CUDA plugin present but provider DLL won't load on this system; \
                 falling back to DirectML."
                    .to_string(),
            );
        } else {
            record_diag(
                "[ort] User selected CUDA but plugin/CUDA/cuDNN not fully present; \
                 falling back to DirectML."
                    .to_string(),
            );
        }
    }

    // Step 2: honor an explicitly-set dev path (env.yaml).
    if let Some(dev_path) = std::env::var_os("ORT_DYLIB_PATH") {
        record_diag(format!(
            "[ort] Active runtime: {} (dev ORT_DYLIB_PATH, requested EP: {:?})",
            std::path::Path::new(&dev_path).display(),
            preferred
        ));
        return;
    }

    // Step 3: bundled DirectML fallback next to exe.
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::PathBuf::from))
    else {
        record_diag("[ort] could not resolve exe dir; leaving ORT_DYLIB_PATH unset".to_string());
        return;
    };
    let bundled = exe_dir
        .join("resources")
        .join("ort")
        .join("directml")
        .join("onnxruntime.dll");
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
    // Linux: installer bundles the CUDA ORT build; env.yaml ORT_DYLIB_PATH is set
    // by CI or the dev's ensure-ort.mjs script. Nothing to select dynamically.
}

/// Prepend `CUDA_PATH\bin` and `CUDNN_PATH\bin\<ver>\x64` to the process `PATH` so
/// Windows' DLL loader can resolve `onnxruntime_providers_cuda.dll`'s dependencies
/// (`cudart64_*.dll`, `cublas64_*.dll`, `cudnn64_9.dll`, etc).
///
/// The CUDA installer adds `%CUDA_PATH%\bin` to PATH automatically, but the cuDNN
/// installer does NOT add its own bin dir - and even on the CUDA side, a freshly-
/// launched dev process may have started before Explorer re-read the PATH env.
/// Fixing it here at runtime is the least-surprise option.
///
/// Must run before `preload_ort_providers()` so the CUDA provider DLL's deps
/// resolve when we LoadLibrary it.
#[cfg(all(feature = "cuda", target_os = "windows"))]
pub fn extend_path_for_cuda_deps() {
    let mut added: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(cuda_path) = std::env::var("CUDA_PATH") {
        let bin = std::path::Path::new(&cuda_path).join("bin");
        if bin.is_dir() {
            added.push(bin);
        }
    }

    if let Ok(cudnn_path) = std::env::var("CUDNN_PATH") {
        // cuDNN 9.x: <CUDNN_PATH>/bin/<cuda_ver>/x64/cudnn64_9.dll
        let pattern = format!(
            "{}/bin/**/cudnn64_*.dll",
            cudnn_path.trim_end_matches(['/', '\\'])
        );
        if let Ok(iter) = glob::glob(&pattern) {
            if let Some(dll) = iter.flatten().next() {
                if let Some(parent) = dll.parent() {
                    added.push(parent.to_path_buf());
                }
            }
        }
        // Fallback: legacy flat `<CUDNN_PATH>/bin/`.
        let flat = std::path::Path::new(&cudnn_path).join("bin");
        if flat.is_dir() && !added.iter().any(|p| p == &flat) {
            added.push(flat);
        }
    }

    if added.is_empty() {
        return;
    }

    let prefix = added
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(";");
    let existing = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{prefix};{existing}");
    eprintln!("[ort] Prepended to PATH for CUDA deps: {prefix}");
    // SAFETY: set_var is unsafe only due to multi-threaded races. Runs before
    // tokio/tauri spawn threads.
    unsafe {
        std::env::set_var("PATH", new_path);
    }
}

#[cfg(not(all(feature = "cuda", target_os = "windows")))]
pub fn extend_path_for_cuda_deps() {}

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

    // Provider DLLs that MUST live next to onnxruntime.dll. `shared` always,
    // `cuda` only when the CUDA runtime is installed (plugin active).
    #[cfg(target_os = "windows")]
    let next_to_ort: &[&str] = &[
        "onnxruntime_providers_shared.dll",
        "onnxruntime_providers_cuda.dll",
    ];
    // TODO(cuda-linux): re-add "libonnxruntime_providers_cuda.so" here once the
    // preload can publish `shared`'s symbols globally. Preloading it from this
    // list can never succeed on Linux: `preload_dylib` uses RTLD_LOCAL, so
    // `libonnxruntime_providers_shared.so`'s symbols stay out of the global
    // namespace, and `providers_cuda.so` does not name `providers_shared` in its
    // DT_NEEDED - it expects those symbols to already be globally visible. The
    // load therefore always failed with `undefined symbol: Provider_GetHost`,
    // printing a scary error on every startup for a no-op.
    //
    // This is Linux-only: on Windows each PE import names its source DLL, so the
    // loader resolves `Provider_GetHost` from providers_shared.dll directly, and
    // macOS ships no CUDA provider at all.
    //
    // Dropping it here costs nothing today - ORT loads the CUDA EP itself at
    // session creation (`with_auto_device`), which is the path that actually
    // decides whether CUDA is used.
    #[cfg(target_os = "linux")]
    let next_to_ort: &[&str] = &["libonnxruntime_providers_shared.so"];
    #[cfg(target_os = "macos")]
    let next_to_ort: &[&str] = &["libonnxruntime_providers_shared.dylib"];

    for name in next_to_ort {
        let path = ort_dir.join(name);
        if !path.exists() {
            // CUDA provider is optional (missing on DirectML runtime) - don't warn.
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

/// Preload CUDA transitive deps (cuDNN, cudart, cublas, ...) by absolute path
/// so their handles exist before the CUDA EP DLL is loaded. Windows' loader
/// uses `LOAD_WITH_ALTERED_SEARCH_PATH` for our preload, which skips PATH for
/// transitive deps but reuses already-loaded DLLs by name.
#[cfg(all(feature = "cuda", target_os = "windows"))]
pub fn preload_cuda_deps() {
    use glob::glob;

    let mut dep_dlls: Vec<std::path::PathBuf> = Vec::new();

    if let Ok(cudnn_path) = std::env::var("CUDNN_PATH") {
        let pattern = format!(
            "{}/bin/**/cudnn*.dll",
            cudnn_path.trim_end_matches(['/', '\\'])
        );
        if let Ok(iter) = glob(&pattern) {
            dep_dlls.extend(iter.flatten());
        }
    }

    if let Ok(cuda_path) = std::env::var("CUDA_PATH") {
        // Recursive so we catch CUDA 13's `bin/x64/*.dll` layout AND CUDA 12's flat `bin/*.dll`.
        let cu_pattern = format!("{}/bin/**/*.dll", cuda_path.trim_end_matches(['/', '\\']));
        if let Ok(iter) = glob(&cu_pattern) {
            for p in iter.flatten() {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if name.starts_with("cudart")
                    || name.starts_with("cublas")
                    || name.starts_with("cusparse")
                    || name.starts_with("curand")
                    || name.starts_with("cusolver")
                    || name.starts_with("cufft")
                {
                    dep_dlls.push(p);
                }
            }
        }
    }

    for path in dep_dlls {
        match ort::util::preload_dylib(&path) {
            Ok(()) => eprintln!("[ORT preload] loaded CUDA dep {}", path.display()),
            Err(e) => eprintln!(
                "[ORT preload] CUDA dep failed for {}: {}",
                path.display(),
                e
            ),
        }
    }
}

#[cfg(not(all(feature = "cuda", target_os = "windows")))]
pub fn preload_cuda_deps() {}
