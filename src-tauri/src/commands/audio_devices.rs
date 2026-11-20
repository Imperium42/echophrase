// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use crate::commands::transcription::log_debug;
use std::process::Command;

/// Check whether a microphone is available on the current platform.
pub async fn check_microphone_available() -> Result<bool, String> {
    // In WSL, check if WSLg PulseAudio is available
    if echo_audio::wsl::is_wsl() {
        return check_microphone_wslg();
    }

    check_microphone_native()
}

fn check_microphone_wslg() -> Result<bool, String> {
    if std::path::Path::new(echo_audio::wsl::WSLG_PULSE_SERVER).exists() {
        if let Ok(output) = Command::new("pactl")
            .env("PULSE_SERVER", echo_audio::wsl::WSLG_PULSE_SERVER)
            .args(&["list", "sources", "short"])
            .output()
        {
            let sources = String::from_utf8_lossy(&output.stdout);
            if sources.contains(echo_audio::wsl::RDP_SOURCE) {
                log::info!("WSLg microphone source detected");
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(target_os = "linux")]
fn check_microphone_native() -> Result<bool, String> {
    if let Ok(output) = Command::new("pactl")
        .args(&["list", "short", "sources"])
        .output()
    {
        let sources = String::from_utf8_lossy(&output.stdout);
        if sources.lines().any(|line| !line.contains(".monitor")) {
            return Ok(true);
        }
    }

    if let Ok(output) = Command::new("arecord").arg("-l").output() {
        let devices = String::from_utf8_lossy(&output.stdout);
        if devices.contains("card") {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(target_os = "windows")]
fn check_microphone_native() -> Result<bool, String> {
    Ok(true)
}

/// On macOS, mic availability is governed by TCC authorization status alone.
///
/// We deliberately do NOT enumerate via `cpal` here: on Apple Silicon the
/// CoreAudio HAL has a race where `host.input_devices()` returns an empty
/// iterator for a short window after `AVCaptureDevice.authorizationStatus`
/// flips to `Authorized`. The user clicks Allow, the status is Authorized,
/// but cpal still sees no inputs - so we'd falsely report "Microphone not
/// detected". Trust TCC: if the user authorized us, the OS-level permission
/// to capture audio exists, and any real device-missing condition will be
/// surfaced (with a much better error) when the recorder
/// tries to open the input stream.
#[cfg(target_os = "macos")]
fn check_microphone_native() -> Result<bool, String> {
    use crate::macos_permissions::{MicAuthStatus, current_status};
    Ok(matches!(current_status(), MicAuthStatus::Authorized))
}

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
fn check_microphone_native() -> Result<bool, String> {
    Ok(true)
}

/// Get the current microphone authorization status without prompting.
///
/// Returns one of: `"authorized"`, `"denied"`, `"restricted"`, `"not_determined"`.
/// On non-macOS platforms this always returns `"authorized"` since permission
/// is managed elsewhere (or not required at all).
#[tauri::command]
pub async fn get_microphone_permission_status() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::macos_permissions::current_status()
            .as_str()
            .to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok("authorized".to_string())
    }
}

/// Trigger the OS microphone permission prompt if undecided.
///
/// On macOS, calls `AVCaptureDevice.requestAccess(for: .audio)`. The first time
/// this is invoked the user sees the system "Echophrase wants to access the
/// microphone" dialog; subsequent calls are no-ops that just return current state.
///
/// Returns the post-request status: `"authorized"` / `"denied"` / `"restricted"`
/// / `"not_determined"`.
#[tauri::command]
pub async fn request_microphone_permission() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let status = tokio::task::spawn_blocking(crate::macos_permissions::request_access_blocking)
            .await
            .map_err(|e| format!("permission task join error: {}", e))?;
        Ok(status.as_str().to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok("authorized".to_string())
    }
}

/// Open the OS-level microphone privacy pane so the user can flip the toggle.
///
/// macOS: jumps directly to System Settings > Privacy & Security > Microphone
/// via the documented `x-apple.systempreferences:` URL scheme. Other platforms
/// return an error (no equivalent deep-link).
#[tauri::command]
pub async fn open_microphone_system_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
            .status()
            .map_err(|e| format!("failed to spawn `open`: {}", e))?;
        if !status.success() {
            return Err(format!("`open` exited with status {}", status));
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("open_microphone_system_settings is only supported on macOS".to_string())
    }
}

/// Get the list of available audio input devices.
///
/// Platform behavior:
/// - Windows: Uses cpal (native WASAPI)
/// - WSL: Uses pactl (PulseAudio via WSLg)
/// - Linux (native): Uses cpal with pactl fallback
/// - macOS: Uses AVFoundation (avoids cpal's post-grant CoreAudio HAL race),
///   falls back to cpal if AVFoundation returns empty.
#[allow(unused_assignments)]
pub async fn get_audio_input_devices() -> Result<Vec<serde_json::Value>, String> {
    if echo_audio::wsl::is_wsl() {
        return get_audio_devices_pactl();
    }

    #[cfg(target_os = "macos")]
    {
        let av_devices = get_audio_devices_avfoundation();
        if !av_devices.is_empty() {
            return Ok(av_devices);
        }
        // AVFoundation came back empty (no hardware, or query failed). Fall
        // through to cpal as a best-effort second opinion before giving up.
        return get_audio_devices_cpal();
    }

    #[cfg(windows)]
    {
        get_audio_devices_cpal()
    }

    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        match get_audio_devices_cpal() {
            Ok(devices) if !devices.is_empty() => Ok(devices),
            _ => get_audio_devices_pactl(),
        }
    }
}

/// Enumerate audio inputs via AVFoundation on macOS.
///
/// Unlike cpal's CoreAudio backend, AVFoundation does not have a post-grant
/// race window - devices appear immediately after TCC flips to Authorized.
/// See `macos_permissions::list_audio_input_devices` for the AVFoundation
/// bridge details.
#[cfg(target_os = "macos")]
fn get_audio_devices_avfoundation() -> Vec<serde_json::Value> {
    use serde_json::json;
    let devices: Vec<serde_json::Value> = crate::macos_permissions::list_audio_input_devices()
        .into_iter()
        .map(|d| json!({ "id": d.id, "name": d.name }))
        .collect();
    log_debug(format!(
        "🎤 [avfoundation] Found {} audio input devices",
        devices.len()
    ));
    devices
}

/// Get audio devices using cpal (Windows WASAPI / macOS CoreAudio).
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn get_audio_devices_cpal() -> Result<Vec<serde_json::Value>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    use serde_json::json;

    let host = cpal::default_host();
    let devices: Vec<_> = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate input devices: {}", e))?
        .filter_map(|device| {
            // cpal 0.18: names live on DeviceDescription (device.name() is gone).
            let name = device.description().ok()?.name().to_string();
            Some(json!({ "id": name.clone(), "name": name }))
        })
        .collect();

    log_debug(format!(
        "🎤 [cpal] Found {} audio input devices",
        devices.len()
    ));
    Ok(devices)
}

/// On Linux we prefer pactl for better PulseAudio/PipeWire integration.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn get_audio_devices_cpal() -> Result<Vec<serde_json::Value>, String> {
    Err("cpal not preferred on this platform".to_string())
}

/// Get audio devices using pactl (Linux/WSL).
#[allow(unused_assignments)]
fn get_audio_devices_pactl() -> Result<Vec<serde_json::Value>, String> {
    use serde_json::json;

    let mut devices = Vec::new();

    let pulse_server = if echo_audio::wsl::is_wsl() {
        Some(echo_audio::wsl::WSLG_PULSE_SERVER)
    } else {
        None
    };

    let mut cmd = Command::new("pactl");
    if let Some(server) = pulse_server {
        cmd.env("PULSE_SERVER", server);
    }
    cmd.args(&["list", "sources"]);

    if let Ok(output) = cmd.output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut current_name: Option<String> = None;
        let mut current_desc: Option<String> = None;

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Name:") {
                current_name = Some(trimmed.trim_start_matches("Name:").trim().to_string());
            } else if trimmed.starts_with("Description:") {
                current_desc = Some(
                    trimmed
                        .trim_start_matches("Description:")
                        .trim()
                        .to_string(),
                );

                if let Some(ref name) = current_name {
                    if !name.contains(".monitor") {
                        let display_name = if name == echo_audio::wsl::RDP_SOURCE
                            || current_desc.as_deref() == Some("RDP Source")
                        {
                            "RDP Source (WSL Passthrough)".to_string()
                        } else {
                            current_desc.clone().unwrap_or_else(|| name.clone())
                        };
                        devices.push(json!({ "id": name, "name": display_name }));
                    }
                }
                current_name = None;
                current_desc = None;
            }
        }

        log_debug(format!(
            "🎤 [pactl] Found {} audio input devices",
            devices.len()
        ));
    } else {
        log_debug("⚠️ Failed to enumerate audio devices via pactl".to_string());
    }

    Ok(devices)
}
