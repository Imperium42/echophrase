// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! macOS dictation-key takeover.
//!
//! On modern Apple keyboards the dedicated **Dictation key** (F5, the mic-icon
//! key) emits a HID Consumer-page usage `0xc000000cf` ("Voice Command") that
//! macOS routes straight into its built-in Dictation service - a normal
//! `RegisterEventHotKey`/global-shortcut binding never sees it.
//!
//! Rather than fight that with an event tap (which the OS handles above the tap
//! layer until the user disables system Dictation), we remap the key at the HID
//! layer with `hidutil` to an otherwise-unused key (**F13**, usage
//! `0x700000068`). The app binds F13 as a normal global shortcut and displays it
//! as "F5". Properties:
//!   - the user never has to unbind macOS Dictation in System Settings,
//!   - it only takes effect while the remap is on,
//!   - no special permission, no driver, no restart (a `hidutil` remap takes
//!     effect on the next keypress and is cleared on reboot anyway).
//!
//! The remap is **driven by the binding** (see `set_dictation_key_remap`): the
//! frontend enables it when the configured hotkey is F13 (the mic key) and
//! reverts it otherwise, so macOS native dictation is only disabled while the
//! user actually has the mic key bound.
//!
//! CAVEAT: `revert` clears the WHOLE `hidutil` UserKeyMapping, so it would
//! clobber any other `hidutil` remap the user set this session. Acceptable for
//! now; a fuller version would read-merge-write the existing mapping.

/// HID usage for the dedicated Dictation / "Voice Command" key (Consumer page).
#[cfg(target_os = "macos")]
const DICTATION_KEY_SRC: &str = "0xc000000cf";

/// HID usage for F13 (Keyboard page 0x07, usage 0x68) - our unused landing key.
#[cfg(target_os = "macos")]
const F13_KEY_DST: &str = "0x700000068";

/// Remap the dictation key to F13 so the app can bind it as a global shortcut.
/// Best-effort: logs and swallows any failure (the app still runs without it).
#[cfg(target_os = "macos")]
pub fn apply_remap() {
    let mapping = format!(
        r#"{{"UserKeyMapping":[{{"HIDKeyboardModifierMappingSrc":{src},"HIDKeyboardModifierMappingDst":{dst}}}]}}"#,
        src = DICTATION_KEY_SRC,
        dst = F13_KEY_DST,
    );
    match std::process::Command::new("hidutil")
        .args(["property", "--set", &mapping])
        .output()
    {
        Ok(out) if out.status.success() => {
            log::info!(
                "dictation-key: remapped {} -> F13 ({})",
                DICTATION_KEY_SRC,
                F13_KEY_DST
            );
        }
        Ok(out) => log::warn!(
            "dictation-key: hidutil remap failed (status {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        Err(e) => log::warn!("dictation-key: could not run hidutil: {}", e),
    }
}

/// Clear our `hidutil` remap so the dictation key reverts to system Dictation.
/// Best-effort; called when the binding leaves F13 and on quit.
#[cfg(target_os = "macos")]
pub fn revert_remap() {
    match std::process::Command::new("hidutil")
        .args(["property", "--set", r#"{"UserKeyMapping":[]}"#])
        .output()
    {
        Ok(out) if out.status.success() => log::info!("dictation-key: remap reverted"),
        Ok(out) => log::warn!(
            "dictation-key: hidutil revert failed (status {:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        Err(e) => log::warn!("dictation-key: could not run hidutil on revert: {}", e),
    }
}

/// Enable or disable the dictation-key -> F13 remap.
///
/// Frontend calls this with `enabled = true` while the user is binding the mic
/// key (so it's capturable as F13) and whenever the saved hotkey is F13;
/// `enabled = false` re-enables macOS native dictation. No-op off macOS.
#[tauri::command]
pub fn set_dictation_key_remap(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        if enabled {
            apply_remap();
        } else {
            revert_remap();
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = enabled;
        Ok(())
    }
}
