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

    // APTABASE_APP_KEY is needed at compile time (env! macro in telemetry/mod.rs),
    // but ONLY for official builds (`--features official`). Community builds
    // compile no telemetry client and need no key.
    // - CI / `bun run tauri:dev` set it explicitly via the environment
    // - Bare `cargo check --features official` falls back to env.yaml so
    //   contributors with the key don't have to source it manually.
    if std::env::var_os("CARGO_FEATURE_OFFICIAL").is_some() {
        match std::env::var("APTABASE_APP_KEY") {
            Ok(key) if !key.is_empty() => {}
            _ => match read_aptabase_key_from_env_yaml() {
                Some(key) => println!("cargo:rustc-env=APTABASE_APP_KEY={}", key),
                // Never fail a release over analytics: build anyway with an
                // empty key, which makes the client drop every event unsent.
                None => {
                    println!(
                        "cargo:warning=`official` build without APTABASE_APP_KEY: \
                         telemetry will be disabled in this binary. Set it in the \
                         environment or in ../env.yaml (shared.APTABASE_APP_KEY)."
                    );
                    println!("cargo:rustc-env=APTABASE_APP_KEY=");
                }
            },
        }
    }
    // Tell cargo to rebuild if env.yaml changes (so the value stays in sync)
    println!("cargo:rerun-if-changed=../env.yaml");
    println!("cargo:rerun-if-env-changed=APTABASE_APP_KEY");

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

/// Hand-parse `../env.yaml` looking for `APTABASE_APP_KEY: "..."`.
///
/// We avoid adding `serde_yaml` as a build-dependency for one string lookup.
/// The line we want lives in the top-level `shared:` block:
///
/// ```yaml
/// shared:
///   APTABASE_APP_KEY: "A-XX-..."
/// ```
fn read_aptabase_key_from_env_yaml() -> Option<String> {
    let contents = std::fs::read_to_string("../env.yaml").ok()?;
    for line in contents.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("APTABASE_APP_KEY:") {
            // Drop any inline `# comment`, then quotes and whitespace. The
            // template's placeholder counts as "no key" (warn + telemetry off).
            let value = rest
                .split('#')
                .next()
                .unwrap_or_default()
                .trim()
                .trim_matches(|c| c == '"' || c == '\'');
            if !value.is_empty() && !value.contains("PLACEHOLDER") {
                return Some(value.to_string());
            }
        }
    }
    None
}
