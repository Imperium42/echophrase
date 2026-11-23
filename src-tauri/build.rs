// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // Set build timestamp for debug info (friendly format: "Jan 06, 2026 5:30 PM")
    let now = chrono::Local::now();
    println!(
        "cargo:rustc-env=BUILD_DATE={}",
        now.format("%b %d, %Y %I:%M %p")
    );

    // On Windows, replace Tauri's default app manifest with ours so we can
    // declare longPathAware (HF cache paths > 260 chars under OneDrive KFM)
    // and UTF-8 active code page (non-ASCII Windows profile names).
    //
    // tauri-build embeds its manifest only into bin targets, so the lib test
    // exe got none, loaded comctl32 v5 and died with STATUS_ENTRYPOINT_NOT_FOUND
    // (TaskDialogIndirect is v6-only). Embedding via linker args instead covers
    // every linked target: the app, test exes and examples.
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let attrs = if target_os == "windows" && target_env == "msvc" {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows/main.manifest");
        println!("cargo:rerun-if-changed=windows/main.manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    } else if target_os == "windows" {
        println!("cargo:rerun-if-changed=windows/main.manifest");
        tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new()
                .app_manifest(include_str!("windows/main.manifest")),
        )
    } else {
        tauri_build::Attributes::new()
    };

    tauri_build::try_build(attrs).expect("failed to run tauri_build::try_build");
}
