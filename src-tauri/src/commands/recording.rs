// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use crate::audio::AudioRecorder;
use crate::tray_status::{self, StatusGuard, TrayStatus};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

// State to hold the recorder instance
pub struct RecorderState(pub Arc<Mutex<AudioRecorder>>);

#[tauri::command]
pub async fn start_audio_recording(
    state: State<'_, RecorderState>,
    app: AppHandle,
) -> Result<(), String> {
    let recorder = state.0.lock().await;
    let result = recorder.start_recording(app.clone()).await;
    if result.is_ok() {
        crate::telemetry::mark_activity();
        tray_status::set(&app, TrayStatus::Recording);
        warm_model_if_idle_unloaded(&app);
    }
    result
}

/// If the model was unloaded for idleness, reload it in the background now so
/// the load overlaps with the user speaking instead of stalling the transcript.
fn warm_model_if_idle_unloaded(app: &AppHandle) {
    if !crate::transcription::idle::on_recording_start() {
        return;
    }
    log::info!("[idle] model was unloaded while idle - reloading in the background");
    let warm_app: AppHandle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::commands::transcription::preload_whisper_model(warm_app).await {
            log::warn!("[idle] background model reload failed: {e}");
        }
    });
}

#[tauri::command]
pub async fn stop_audio_recording_and_transcribe(
    state: State<'_, RecorderState>,
    app: AppHandle,
) -> Result<String, String> {
    let recorder = state.0.lock().await;

    // Get raw audio samples directly from memory
    let samples = recorder.stop_recording_raw().await?;
    // Release the recorder lock before kicking off transcription. Whisper /
    // Parakeet inference can take seconds; holding this lock blocks every
    // other recorder command (reset, status checks) for the full inference.
    drop(recorder);
    // Recording stopped; tray flips to Transcribing for the duration of the
    // model run, then Drop returns it to Idle on every exit path.
    let _tray_guard = StatusGuard::new(app.clone(), TrayStatus::Transcribing, TrayStatus::Idle);

    // Captured samples are already 16 kHz mono - wrap as an in-memory source
    // and let the shared router pick the backend (Candle gets preview events).
    let source = echo_audio::AudioSource::from_samples_16k(samples);
    let transcript = crate::transcription::transcribe_source(&source, Some(&app))
        .await
        .map_err(|e| format!("Transcription failed: {}", e))?;

    Ok(transcript)
}

/// Start recording with inline VAD monitoring for open mic mode.
/// VAD runs on each 32ms audio chunk, emitting events when speech segments complete.
#[tauri::command]
pub async fn start_audio_recording_with_vad(
    state: State<'_, RecorderState>,
    app: AppHandle,
    threshold: f32,
    silence_timeout_ms: u64,
) -> Result<(), String> {
    let recorder = state.0.lock().await;
    let result = recorder
        .start_recording_with_vad(app.clone(), threshold, silence_timeout_ms)
        .await;
    if result.is_ok() {
        crate::telemetry::mark_activity();
        tray_status::set(&app, TrayStatus::Recording);
        warm_model_if_idle_unloaded(&app);
    }
    result
}

/// Result of open mic chunk transcription.
///
/// Includes audio duration so the frontend can accurately bill user
/// recording time against daily/monthly quotas instead of counting
/// transcription inference time (which varies with backend and GPU).
#[derive(Debug, serde::Serialize)]
pub struct OpenMicChunkResult {
    /// The transcribed text. Empty string if audio was too quiet or empty.
    pub text: String,
    /// Length of the input audio in milliseconds. Includes the trailing
    /// silence that triggered chunk emission; callers should subtract their
    /// configured silence_timeout to get the billable speech duration.
    pub audio_duration_ms: u32,
}

/// Transcribe the pending buffer from open mic chunk detection.
/// Recording continues uninterrupted. Returns transcribed text and audio duration.
#[tauri::command]
pub async fn transcribe_open_mic_chunk(
    state: State<'_, RecorderState>,
    app: AppHandle,
) -> Result<OpenMicChunkResult, String> {
    let recorder = state.0.lock().await;
    let samples = recorder
        .take_pending_buffer()
        .await?
        .ok_or("No pending buffer available")?;
    drop(recorder); // Release lock before transcription

    // Compute audio duration up-front so every return path reports it.
    // Parakeet/Whisper both expect 16kHz mono f32.
    let audio_duration_ms = ((samples.len() as u64) * 1000 / 16000) as u32;

    if samples.is_empty() {
        return Ok(OpenMicChunkResult {
            text: String::new(),
            audio_duration_ms,
        });
    }

    // Guard: reject near-silent audio to prevent Whisper hallucinations
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    const MIN_RMS_THRESHOLD: f32 = 0.003; // ~-50 dBFS
    if rms < MIN_RMS_THRESHOLD {
        log::info!(
            "[OpenMic] Skipping transcription - audio too quiet (RMS: {:.4})",
            rms
        );
        return Ok(OpenMicChunkResult {
            text: String::new(),
            audio_duration_ms,
        });
    }

    // Track GPU backend once per session on first transcription
    crate::telemetry::track_gpu_backend(&app);

    // Flip tray to Transcribing for the model run, then back to Recording
    // (open mic recording continues across chunks). Drop on every exit path
    // ensures a transcription error doesn't leave the tray stuck.
    let _tray_guard =
        StatusGuard::new(app.clone(), TrayStatus::Transcribing, TrayStatus::Recording);

    // Same shared router as stop_audio_recording_and_transcribe: in-memory
    // 16 kHz source, backend chosen from settings, Candle preview enabled.
    let source = echo_audio::AudioSource::from_samples_16k(samples);
    let transcript = crate::transcription::transcribe_source(&source, Some(&app))
        .await
        .map_err(|e| format!("Transcription failed: {}", e))?;

    Ok(OpenMicChunkResult {
        text: transcript,
        audio_duration_ms,
    })
}

/// Force reset recording state - recovers from stuck "Already recording" errors
#[tauri::command]
pub async fn reset_recording_state(state: State<'_, RecorderState>) -> Result<(), String> {
    let recorder = state.0.lock().await;
    recorder.reset_recording_state().await
}

#[tauri::command]
pub async fn is_wsl_environment() -> Result<bool, String> {
    // On Windows, we're never in WSL
    #[cfg(target_os = "windows")]
    {
        Ok(false)
    }
    // On Linux, check for WSL indicators
    #[cfg(not(target_os = "windows"))]
    {
        Ok(std::env::var("WSL_DISTRO_NAME").is_ok()
            || std::env::var("WSL_INTEROP").is_ok()
            || std::path::Path::new("/mnt/wslg/PulseServer").exists())
    }
}

/// Get system information for debug purposes
#[tauri::command]
pub async fn get_system_info() -> Result<SystemInfo, String> {
    #[cfg(target_os = "macos")]
    use std::process::Command;

    // Windows builds are NEVER running in WSL
    #[cfg(target_os = "windows")]
    let is_wsl = false;

    #[cfg(not(target_os = "windows"))]
    let is_wsl = std::env::var("WSL_DISTRO_NAME").is_ok()
        || std::env::var("WSL_INTEROP").is_ok()
        || std::path::Path::new("/mnt/wslg/PulseServer").exists();

    let os = if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if is_wsl {
        "Linux (WSL)".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else {
        std::env::consts::OS.to_string()
    };

    // Probe GPU info via NVML in-process (Windows/Linux with NVIDIA).
    // Replaces the old `Command::new("nvidia-smi")` shell-out, which on
    // Windows could trigger a STATUS_DLL_INIT_FAILED (0xC0000142) popup
    // when the driver was in a bad state. NVML init returns Err (no popup)
    // in the same scenarios. macOS falls through to system_profiler below.
    // mut needed by the macOS system_profiler fallback below; allow on other targets.
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let (mut gpu_name, mut gpu_vram_mb) = match crate::gpu_probe::primary_gpu_info() {
        Some((name, vram_mb)) => (Some(name), Some(vram_mb)),
        None => (None, None),
    };

    // macOS: nvidia-smi doesn't exist - use system_profiler to detect Metal GPU
    #[cfg(target_os = "macos")]
    if gpu_name.is_none() {
        if let Ok(output) = Command::new("system_profiler")
            .arg("SPDisplaysDataType")
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // Parse "Chipset Model: Intel HD Graphics 6000" or "Chipset Model: Apple M1"
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Chipset Model:") {
                        gpu_name = trimmed
                            .strip_prefix("Chipset Model:")
                            .map(|s| s.trim().to_string());
                    }
                    // Parse "VRAM (Total): 1536 MB" (Intel) or "VRAM (Dynamic, Max): 2048 MB"
                    if trimmed.starts_with("VRAM") && trimmed.contains("MB") {
                        if let Some(mb_str) = trimmed.split(':').nth(1) {
                            let digits: String =
                                mb_str.chars().filter(|c| c.is_ascii_digit()).collect();
                            gpu_vram_mb = digits.parse::<u32>().ok();
                        }
                    }
                }

                // Apple Silicon reports unified memory as "VRAM (Dynamic, Max)" which
                // inflates model tier classification. Cap to a conservative value for
                // model selection since unified memory is shared with the system.
                if let Some(ref name) = gpu_name {
                    if name.starts_with("Apple M") {
                        // Conservative cap: Apple Silicon shares unified memory with the OS,
                        // so report 8GB as effective VRAM for model selection purposes.
                        // This prevents auto-selecting models too large for comfortable use.
                        if let Some(vram) = gpu_vram_mb {
                            if vram > 8192 {
                                log::info!(
                                    "Apple Silicon ({}) reports {}MB unified memory, capping to 8192MB for model selection",
                                    name,
                                    vram
                                );
                                gpu_vram_mb = Some(8192);
                            }
                        }
                    }
                }
            }
        }
    }

    // Build timestamp from compile time
    let build_date = env!("CARGO_PKG_VERSION").to_string() + " (" + env!("BUILD_DATE") + ")";

    let cpu_arch = std::env::consts::ARCH.to_string();

    // Detect Linux distro from /etc/os-release (for NVIDIA download URL preselection)
    let (linux_distro, linux_distro_version) = if cfg!(target_os = "linux") {
        parse_linux_distro()
    } else {
        (None, None)
    };

    Ok(SystemInfo {
        os,
        is_wsl,
        gpu_name,
        gpu_vram_mb,
        build_date,
        cpu_arch,
        linux_distro,
        linux_distro_version,
    })
}

/// System information for debug output
#[derive(serde::Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub is_wsl: bool,
    pub gpu_name: Option<String>,
    pub gpu_vram_mb: Option<u32>,
    pub build_date: String,
    pub cpu_arch: String,
    pub linux_distro: Option<String>,
    pub linux_distro_version: Option<String>,
}

/// Parse /etc/os-release to extract distro name and version.
/// Maps distro names to NVIDIA's expected values (eg "Ubuntu", "Debian").
fn parse_linux_distro() -> (Option<String>, Option<String>) {
    let content = match std::fs::read_to_string("/etc/os-release") {
        Ok(c) => c,
        Err(_) => return (None, None),
    };

    let mut name: Option<String> = None;
    let mut version_id: Option<String> = None;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("NAME=") {
            name = Some(val.trim_matches('"').to_string());
        } else if let Some(val) = line.strip_prefix("VERSION_ID=") {
            version_id = Some(val.trim_matches('"').to_string());
        }
    }

    // Map to NVIDIA's expected distro names
    let nvidia_distro = name.map(|n| {
        let lower = n.to_lowercase();
        if lower.contains("ubuntu") {
            "Ubuntu".to_string()
        } else if lower.contains("debian") {
            "Debian".to_string()
        } else if lower.contains("fedora") {
            "Fedora".to_string()
        } else if lower.contains("rhel") || lower.contains("red hat") {
            "RHEL".to_string()
        } else if lower.contains("opensuse") || lower.contains("suse") {
            "OpenSUSE".to_string()
        } else if lower.contains("rocky") {
            "Rocky".to_string()
        } else if lower.contains("arch") {
            "Arch".to_string()
        } else {
            n
        }
    });

    (nvidia_distro, version_id)
}

/// Check if running in WSL environment
/// Used by frontend to decide whether to use WSL workaround or native opener
/// NOTE: Windows builds ALWAYS return false - a Windows .exe is never "in WSL"
#[tauri::command]
pub fn is_wsl() -> bool {
    // Windows builds are NEVER running in WSL - they're native Windows
    #[cfg(target_os = "windows")]
    {
        false
    }
    // Linux builds check for WSL environment
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("WSL_DISTRO_NAME").is_ok()
            || std::env::var("WSL_INTEROP").is_ok()
            || std::path::Path::new("/mnt/wslg/PulseServer").exists()
    }
}

/// Validates that a URL uses an allowlisted scheme before handing it to an
/// OS opener (xdg-open / ShellExecuteW / cmd.exe start / open). Those openers
/// happily launch `file://` and any registered custom protocol handler, so a
/// compromised or confused frontend must not be able to pass arbitrary schemes.
fn validate_url_scheme(url: &str) -> Result<(), String> {
    const ALLOWED_SCHEMES: &[&str] = &["http://", "https://", "mailto:"];

    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if ALLOWED_SCHEMES.iter().any(|s| lower.starts_with(s)) {
        Ok(())
    } else {
        Err(format!(
            "Refusing to open URL with disallowed scheme (only http/https/mailto): {}",
            trimmed
        ))
    }
}

/// Open URL in the system's default browser, platform-aware
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    validate_url_scheme(&url)?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL on macOS: {}", e))?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        return open_url_windows(url);
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
        return Ok(());
    }
}

/// Open URL in Windows browser from WSL using cmd.exe /c start
/// WSL's interop converts argv to a Windows command line using MSVC quoting,
/// which only adds double-quotes for args containing spaces/tabs. Since URLs
/// rarely have spaces, & would pass through unquoted and cmd.exe would interpret
/// it as a command separator. We escape cmd.exe metacharacters with ^ (caret).
/// The empty title arg prevents start from treating the URL as a window title.
/// current_dir("/mnt/c") avoids the UNC path error (\\wsl.localhost\...) that
/// kills cmd.exe when CWD is inside the WSL filesystem.
/// Falls back to rundll32 FileProtocolHandler if cmd.exe fails.
#[tauri::command]
pub async fn open_url_wsl(url: String) -> Result<(), String> {
    use std::process::Command;

    validate_url_scheme(&url)?;
    log::info!("Opening URL in Windows browser (WSL): {}", url);

    // Escape cmd.exe metacharacters with ^ so they pass through literally
    let escaped = url
        .replace('&', "^&")
        .replace('|', "^|")
        .replace('<', "^<")
        .replace('>', "^>");

    let status = Command::new("cmd.exe")
        .arg("/c")
        .arg("start")
        .arg("")
        .arg(&escaped)
        .current_dir("/mnt/c")
        .status()
        .map_err(|e| format!("Failed to launch cmd.exe: {}", e))?;

    if !status.success() {
        log::warn!(
            "cmd.exe start failed (exit {}), trying rundll32 fallback",
            status
        );
        Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(&url)
            .current_dir("/mnt/c")
            .spawn()
            .map_err(|e| format!("Failed to open URL in Windows browser: {}", e))?;
    }

    Ok(())
}

/// Open URL using xdg-open (native Linux, not WSL)
#[tauri::command]
pub async fn open_url_linux(url: String) -> Result<(), String> {
    use std::process::Command;

    validate_url_scheme(&url)?;
    log::info!("Opening URL via xdg-open: {}", url);
    Command::new("xdg-open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open URL via xdg-open: {}", e))?;

    Ok(())
}

/// Open URL using macOS `open` command
#[tauri::command]
pub async fn open_url_mac(url: String) -> Result<(), String> {
    use std::process::Command;

    validate_url_scheme(&url)?;
    log::info!("Opening URL via open (macOS): {}", url);
    Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open URL via open: {}", e))?;

    Ok(())
}

/// Open URL using Windows native ShellExecuteW API (no console window)
/// This bypasses tauri-plugin-opener which spawns cmd.exe on Windows
#[cfg(target_os = "windows")]
#[tauri::command]
pub fn open_url_windows(url: String) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    validate_url_scheme(&url)?;

    // Convert URL to wide string (null-terminated UTF-16)
    let wide_url: Vec<u16> = OsStr::new(&url)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let wide_open: Vec<u16> = OsStr::new("open")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let result = windows_sys::Win32::UI::Shell::ShellExecuteW(
            ptr::null_mut(),
            wide_open.as_ptr(),
            wide_url.as_ptr(),
            ptr::null(),
            ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );

        if result as isize > 32 {
            log::info!("Opened URL via ShellExecuteW: {}", url);
            Ok(())
        } else {
            Err(format!(
                "ShellExecuteW failed with code: {}",
                result as isize
            ))
        }
    }
}

/// Stub for non-Windows platforms
#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub fn open_url_windows(_url: String) -> Result<(), String> {
    Err("open_url_windows is only available on Windows".to_string())
}

/// Copy text to Windows clipboard via clip.exe (for WSL environments)
/// This bypasses the Wayland clipboard which doesn't bridge to Windows
#[tauri::command]
pub async fn copy_to_windows_clipboard(text: String) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("clip.exe")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn clip.exe: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to clip.exe: {}", e))?;
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for clip.exe: {}", e))?;

    if status.success() {
        log::info!("📋 Copied to Windows clipboard via clip.exe");
        Ok(())
    } else {
        Err(format!("clip.exe exited with status: {}", status))
    }
}

/// Check if ONNX Runtime is available on this system.
/// On macOS < 13.3, ORT fails to load due to missing C++20 symbols.
/// Frontend uses this to hide ONNX models and show Candle alternatives.
#[tauri::command]
pub fn check_ort_available() -> bool {
    crate::is_onnx_runtime_available()
}

// WAV byte encoding lives in echo_audio::wav::encode_wav_f32_mono_16k
// (used via AudioSource::to_wav_bytes in the transcription router).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_http_https_mailto() {
        assert!(validate_url_scheme("https://echophrase.com").is_ok());
        assert!(validate_url_scheme("http://localhost:3044/health").is_ok());
        assert!(validate_url_scheme("mailto:support@echophrase.com").is_ok());
        assert!(validate_url_scheme("HTTPS://ECHOPHRASE.COM").is_ok());
        assert!(validate_url_scheme("  https://echophrase.com  ").is_ok());
    }

    #[test]
    fn rejects_file_and_custom_schemes() {
        assert!(validate_url_scheme("file:///etc/passwd").is_err());
        assert!(validate_url_scheme("file://C:/Windows/System32/calc.exe").is_err());
        assert!(validate_url_scheme("javascript:alert(1)").is_err());
        assert!(validate_url_scheme("ms-settings:windowsupdate").is_err());
        assert!(validate_url_scheme("smb://attacker/share").is_err());
        assert!(validate_url_scheme("echophrase://auth/success").is_err());
    }

    #[test]
    fn rejects_non_urls() {
        assert!(validate_url_scheme("").is_err());
        assert!(validate_url_scheme("C:\\Windows\\evil.exe").is_err());
        assert!(validate_url_scheme("not a url").is_err());
        // scheme must be a prefix, not merely present
        assert!(validate_url_scheme("evil://x?y=https://echophrase.com").is_err());
    }
}
