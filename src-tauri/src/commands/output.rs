// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use crate::error::ToCmdErr;
use tauri_plugin_dialog::DialogExt;

/// Sentinel error prefix returned by [`simulate_paste`] when macOS has not
/// granted keystroke-posting access. The frontend matches on this to show an
/// explanatory modal instead of a bare "paste failed" toast.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const ACCESSIBILITY_REQUIRED: &str = "ACCESSIBILITY_REQUIRED";

/// Whether the app may synthesize keystrokes into other apps.
///
/// macOS gates this behind `kTCCServicePostEvent` (surfaced in System Settings
/// under Accessibility); every other platform allows it unconditionally, so this
/// returns `true` there.
///
/// Never prompts - see [`crate::macos_permissions::can_post_keyboard_events`].
#[tauri::command]
pub async fn get_paste_permission_granted() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        Ok(crate::macos_permissions::can_post_keyboard_events())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Raise the macOS permission prompt for keystroke posting and report the result.
///
/// This is the correct narrow request (`kTCCServicePostEvent`) rather than full
/// Accessibility. Show our own explanation first: the OS dialog's wording is not
/// ours to control.
#[tauri::command]
pub async fn request_paste_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        // The system dialog blocks until the user decides - keep it off the async
        // runtime's worker threads.
        tokio::task::spawn_blocking(crate::macos_permissions::request_post_event_access)
            .await
            .map_err(|e| format!("permission task join error: {}", e))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Open System Settings > Privacy & Security > Accessibility so the user can add
/// Echophrase to the allow-list.
///
/// macOS only; other platforms have no equivalent pane and return an error.
#[tauri::command]
pub async fn open_accessibility_system_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .status()
            .map_err(|e| format!("failed to spawn `open`: {}", e))?;
        if !status.success() {
            return Err(format!("`open` exited with status {}", status));
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("open_accessibility_system_settings is only supported on macOS".to_string())
    }
}

/// Simulates a paste keystroke (Ctrl+V on Windows/Linux, Cmd+V on macOS)
/// This allows the app to paste clipboard content at the cursor position in any focused app
///
/// On macOS, enigo calls TSMGetInputSourceProperty which must run on the main thread.
/// We use std::thread::spawn + dispatch to avoid crashing from a tokio worker thread.
#[tauri::command]
pub async fn simulate_paste() -> Result<(), String> {
    // Small delay to ensure clipboard is ready and focus is on target window
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    #[cfg(windows)]
    {
        focus::restore_last_external_window().await;
        log::debug!(
            "Paste target (foreground window): {:?}",
            focus::foreground_window_title()
        );
    }

    // Run enigo on a dedicated thread (not tokio worker) to avoid macOS main-thread assertions.
    // On macOS, enigo's key mapping calls HIToolbox APIs that assert dispatch_queue_main.
    // Using spawn_blocking moves it off the async runtime but still isn't the main thread,
    // so on macOS we use dispatch::Queue::main().exec_async() to bounce to the main thread.
    tokio::task::spawn_blocking(do_simulate_paste)
        .await
        .map_err(|e| format!("spawn_blocking join error: {}", e))?
}

/// Paste-target focus tracking (Windows).
///
/// Ctrl+V goes to the foreground window. When that is Echophrase itself (eg
/// the user clicked in the app, then open mic's countdown finished), the paste
/// landed nowhere and the text only sat on the clipboard. A background thread
/// remembers the last foreground window owned by another process, and paste
/// hands focus back to it first.
#[cfg(windows)]
pub mod focus {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::time::Duration;
    use windows_sys::Win32::System::Threading::GetCurrentProcessId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
        SetForegroundWindow,
    };

    const POLL_INTERVAL: Duration = Duration::from_millis(150);
    /// Time for the target window to take focus before Ctrl+V is sent.
    const REFOCUS_SETTLE: Duration = Duration::from_millis(80);

    /// Last foreground HWND owned by another process (0 = none yet).
    static LAST_EXTERNAL: AtomicIsize = AtomicIsize::new(0);

    fn foreground_is_ours() -> Option<isize> {
        // SAFETY: GetForegroundWindow takes no args and may return null;
        // GetWindowThreadProcessId only writes the pid into our local.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid == GetCurrentProcessId() {
                Some(0)
            } else {
                Some(hwnd as isize)
            }
        }
    }

    /// Start the tracker thread. Call once at app setup.
    pub fn start_tracker() {
        std::thread::Builder::new()
            .name("paste-focus-tracker".into())
            .spawn(|| {
                loop {
                    if let Some(hwnd) = foreground_is_ours().filter(|h| *h != 0) {
                        LAST_EXTERNAL.store(hwnd, Ordering::Relaxed);
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
            })
            .map(|_| ())
            .unwrap_or_else(|e| log::warn!("paste focus tracker failed to start: {e}"));
    }

    /// If Echophrase holds focus, give it back to the last external window.
    pub async fn restore_last_external_window() {
        if foreground_is_ours() != Some(0) {
            return;
        }
        let target = LAST_EXTERNAL.load(Ordering::Relaxed);
        if target == 0 {
            log::warn!("Paste: Echophrase has focus and no previous window is known");
            return;
        }
        let hwnd = target as windows_sys::Win32::Foundation::HWND;
        // SAFETY: IsWindow validates the handle (it may have closed since);
        // SetForegroundWindow is allowed because our process is foreground.
        let ok = unsafe { IsWindow(hwnd) != 0 && SetForegroundWindow(hwnd) != 0 };
        if !ok {
            log::warn!("Paste: could not refocus the previous window");
            return;
        }
        tokio::time::sleep(REFOCUS_SETTLE).await;
    }

    /// Title of the current foreground window, for the paste log line.
    pub fn foreground_window_title() -> String {
        // SAFETY: GetWindowTextW writes at most `buf.len()` UTF-16 units.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return "<none>".to_string();
            }
            let mut buf = [0u16; 256];
            let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            String::from_utf16_lossy(&buf[..len.max(0) as usize])
        }
    }
}

fn do_simulate_paste() -> Result<(), String> {
    use enigo::{Enigo, Key, Keyboard, Settings};

    #[cfg(target_os = "macos")]
    {
        // Bail out BEFORE touching enigo when post-event access is missing.
        // CGEventPost would otherwise make macOS raise its own context-free
        // "Echophrase would like to control this computer" dialog, leaving the
        // user with no idea why a dictation app wants that. Returning the
        // sentinel lets the UI explain first (transcript is already on the
        // clipboard at this point, so nothing is lost).
        if !crate::macos_permissions::can_post_keyboard_events() {
            return Err(ACCESSIBILITY_REQUIRED.to_string());
        }

        // On macOS, enigo keyboard APIs must be called from the main thread
        // because they call TSMGetInputSourceProperty which asserts dispatch_queue_main.
        // We use a oneshot channel to get the result back.
        let (tx, rx) = std::sync::mpsc::channel();
        dispatch::Queue::main().exec_async(move || {
            let result = (|| -> Result<(), String> {
                let mut enigo = Enigo::new(&Settings::default()).to_cmd()?;
                enigo.key(Key::Meta, enigo::Direction::Press).to_cmd()?;
                enigo
                    .key(Key::Unicode('v'), enigo::Direction::Click)
                    .to_cmd()?;
                enigo.key(Key::Meta, enigo::Direction::Release).to_cmd()?;
                log::info!("Paste keystroke simulated (Cmd+V)");
                Ok(())
            })();
            let _ = tx.send(result);
        });
        // Wait for the main-thread block to complete (with a timeout to avoid deadlocks)
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| format!("Main thread paste timed out: {}", e))?
    }

    #[cfg(not(target_os = "macos"))]
    {
        // WSL: enigo targets the X11/Wayland display inside WSLg, not the Windows
        // foreground window. We invoke wscript.exe (a GUI-subsystem process) on
        // the Windows side, which does NOT steal foreground focus the way
        // powershell.exe does. The VBS script then SendKeys'es Ctrl+V into
        // whatever window the user was last typing in.
        let is_wsl = std::env::var("WSL_DISTRO_NAME").is_ok()
            || std::env::var("WSL_INTEROP").is_ok()
            || std::path::Path::new("/usr/lib/wsl/lib").exists();

        if is_wsl {
            simulate_paste_wsl()
        } else {
            let mut enigo = Enigo::new(&Settings::default()).to_cmd()?;
            enigo.key(Key::Control, enigo::Direction::Press).to_cmd()?;
            enigo
                .key(Key::Unicode('v'), enigo::Direction::Click)
                .to_cmd()?;
            enigo
                .key(Key::Control, enigo::Direction::Release)
                .to_cmd()?;
            log::info!("Paste keystroke simulated (Ctrl+V)");
            Ok(())
        }
    }
}

/// WSL paste implementation using wscript.exe to launch a hidden PowerShell
/// that calls `user32.dll SendInput` directly.
///
/// ## Why the old SendKeys approach failed
///
/// `WScript.Shell.SendKeys` targets the foreground window at the moment of
/// the call. By the time wscript.exe has started and the VBS executes (~150ms),
/// the Tauri webview has often reclaimed foreground focus - so the keystrokes
/// land in the app itself, or nowhere useful.
///
/// ## Why this approach works
///
/// `user32.dll SendInput` injects a synthetic keystroke into the system's
/// hardware-input queue. The OS delivers it to whichever window currently owns
/// keyboard focus. Crucially, it does NOT require the target window to be in
/// the foreground - it just needs focus, which the user's editor/IDE/browser
/// retains because:
///   1. wscript.exe is a GUI-subsystem process: spawning it from WSL does NOT
///      create a console window or steal foreground focus.
///   2. The hidden PowerShell child launched by the VBS also has no window.
///   3. SendInput fires at the HID driver level, bypassing the foreground check.
///
/// ## Process chain
///
/// WSL (Rust spawn) -> wscript.exe (GUI subsystem, no focus steal)
///   -> powershell.exe -WindowStyle Hidden (no window)
///     -> Add-Type P/Invoke user32!SendInput (injects Ctrl+V at HID level)
#[cfg(not(target_os = "macos"))]
fn simulate_paste_wsl() -> Result<(), String> {
    use std::process::Command;

    // The PowerShell/C# script below P/Invokes user32!SendInput.
    // Struct layout notes (64-bit Windows):
    //   KEYBDINPUT: wVk(2) + wScan(2) + dwFlags(4) + time(4) + dwExtraInfo(8) = 20 bytes
    //   INPUT union: type(4) + padding(4) + union body (max 24 bytes for MOUSEINPUT) = 32 bytes
    // We use [StructLayout(LayoutKind.Explicit)] with explicit field offsets to match
    // the Win32 memory layout exactly, avoiding any marshalling guesswork.
    let ps_script = r#"
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class KbSend {
    [StructLayout(LayoutKind.Explicit)]
    struct INPUT {
        [FieldOffset(0)]  public int    type_;
        [FieldOffset(8)]  public ushort wVk;
        [FieldOffset(10)] public ushort wScan;
        [FieldOffset(12)] public uint   dwFlags;
        [FieldOffset(16)] public uint   time;
        [FieldOffset(20)] public IntPtr dwExtraInfo;
    }
    [DllImport("user32.dll", SetLastError=true)]
    static extern uint SendInput(uint nInputs, INPUT[] pInputs, int cbSize);
    public static void PasteCtrlV() {
        int sz = Marshal.SizeOf(typeof(INPUT));
        INPUT[] inp = new INPUT[4];
        inp[0].type_ = 1; inp[0].wVk = 0x11;                     // Ctrl down
        inp[1].type_ = 1; inp[1].wVk = 0x56;                     // V down
        inp[2].type_ = 1; inp[2].wVk = 0x56; inp[2].dwFlags = 2; // V up
        inp[3].type_ = 1; inp[3].wVk = 0x11; inp[3].dwFlags = 2; // Ctrl up
        SendInput(4, inp, sz);
    }
}
'@
[KbSend]::PasteCtrlV()
"#;

    // Write the PowerShell script to a temp file in the Linux filesystem.
    // We re-write it every invocation so stale versions never linger.
    let ps_wsl_path = "/tmp/echophrase_paste.ps1";
    std::fs::write(ps_wsl_path, ps_script)
        .map_err(|e| format!("Failed to write PS1 helper: {}", e))?;

    // Convert the WSL path to a Windows path so PowerShell can open it.
    let wslpath_out = Command::new("wslpath")
        .args(["-w", ps_wsl_path])
        .output()
        .map_err(|e| format!("Failed to run wslpath: {}", e))?;
    if !wslpath_out.status.success() {
        return Err(format!(
            "wslpath failed: {}",
            String::from_utf8_lossy(&wslpath_out.stderr)
        ));
    }
    let ps_win_path = String::from_utf8_lossy(&wslpath_out.stdout)
        .trim()
        .to_string();

    // Build the VBS launcher: it calls PowerShell with -WindowStyle Hidden
    // so no console or window appears. wscript.exe itself is GUI-subsystem,
    // so spawning it from WSL never steals foreground focus.
    //
    // The VBS uses Shell.Run with bWaitOnReturn=false (fire-and-forget) so
    // wscript exits immediately after launching PS, giving us fast unblock.
    let vbs_content = format!(
        "Set sh = CreateObject(\"WScript.Shell\")\r\n\
         sh.Run \"powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File {}\", 0, False\r\n",
        ps_win_path
    );
    let vbs_wsl_path = "/tmp/echophrase_paste.vbs";
    std::fs::write(vbs_wsl_path, &vbs_content)
        .map_err(|e| format!("Failed to write VBS launcher: {}", e))?;

    let vbs_wslpath_out = Command::new("wslpath")
        .args(["-w", vbs_wsl_path])
        .output()
        .map_err(|e| format!("Failed to run wslpath for VBS: {}", e))?;
    if !vbs_wslpath_out.status.success() {
        return Err(format!(
            "wslpath failed for VBS: {}",
            String::from_utf8_lossy(&vbs_wslpath_out.stderr)
        ));
    }
    let vbs_win_path = String::from_utf8_lossy(&vbs_wslpath_out.stdout)
        .trim()
        .to_string();

    // Spawn wscript.exe detached. current_dir("/mnt/c") avoids UNC path
    // issues (same pattern used by open_url_wsl and copy_to_windows_clipboard).
    Command::new("wscript.exe")
        .args(["//nologo", &vbs_win_path])
        .current_dir("/mnt/c")
        .spawn()
        .map_err(|e| format!("Failed to spawn wscript.exe: {}", e))?;

    log::info!("Paste keystroke dispatched via SendInput (wscript -> hidden PS -> user32, WSL)");
    Ok(())
}

/// Opens a save dialog and writes content to the selected file
/// Returns the saved file path as a string, or None if cancelled
#[tauri::command]
pub async fn save_transcript_dialog(
    app: tauri::AppHandle,
    content: String,
    filename: String,
) -> Result<Option<String>, String> {
    use std::io::Write;

    // Determine file extension based on content
    let extension = if content.trim_start().starts_with('{') {
        "json"
    } else if content.contains(" --> ") {
        "srt"
    } else {
        "txt"
    };

    let default_filename = if filename.contains('.') {
        filename
    } else {
        format!("{}.{}", filename, extension)
    };

    // Use Tauri's dialog API
    let file_path = app
        .dialog()
        .file()
        .add_filter("Text files", &["txt", "json", "srt"])
        .set_file_name(&default_filename)
        .blocking_save_file();

    match file_path {
        Some(file_path) => {
            // Convert FilePath to PathBuf
            let path = std::path::PathBuf::from(file_path.as_path().ok_or("Invalid file path")?);
            // Write content to file
            let mut file = std::fs::File::create(&path).to_cmd()?;
            file.write_all(content.as_bytes()).to_cmd()?;
            let path_str = path.to_string_lossy().to_string();
            log::info!("Transcript saved to: {:?}", path);
            Ok(Some(path_str))
        }
        None => {
            // User cancelled the dialog
            log::info!("Save dialog cancelled by user");
            Ok(None)
        }
    }
}

// delete_temp_file removed: live recording is in-memory now (no temp WAVs).

/// Validates that `filename` is a plain file name with no path components.
/// Rejects separators, `..`, absolute paths, and Windows drive prefixes so a
/// crafted filename cannot escape the target directory (same containment
/// philosophy the removed `delete_temp_file` used).
fn validate_plain_filename(filename: &str) -> Result<(), String> {
    if filename.is_empty() {
        return Err("Filename must not be empty".to_string());
    }
    if filename.contains('/') || filename.contains('\\') || filename.contains(':') {
        return Err(format!(
            "Filename must not contain path separators: {}",
            filename
        ));
    }
    if filename == "." || filename == ".." {
        return Err(format!("Invalid filename: {}", filename));
    }
    // Belt-and-braces: a valid plain filename parses to exactly one Normal component
    let mut components = std::path::Path::new(filename).components();
    match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(_)), None) => Ok(()),
        _ => Err(format!("Invalid filename: {}", filename)),
    }
}

/// Saves transcript content to a file in the configured directory
/// Creates the directory if it doesn't exist. Returns the full saved path.
#[tauri::command]
pub async fn save_transcript_to_directory(
    content: String,
    filename: String,
    directory: String,
) -> Result<String, String> {
    use std::io::Write;

    validate_plain_filename(&filename)?;

    let dir_path = std::path::Path::new(&directory);

    // Create directory if needed
    if !dir_path.exists() {
        std::fs::create_dir_all(dir_path)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Containment check: the canonicalized parent of the target must be the
    // canonicalized destination directory (defends against symlink tricks and
    // any filename parsing edge case the validation above might miss).
    let canonical_dir = dir_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve directory: {}", e))?;

    let file_path = canonical_dir.join(&filename);
    if file_path.parent() != Some(canonical_dir.as_path()) {
        return Err(format!(
            "Refusing to write outside target directory: {}",
            file_path.display()
        ));
    }
    let mut file =
        std::fs::File::create(&file_path).map_err(|e| format!("Failed to create file: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write file: {}", e))?;

    let path_str = file_path.to_string_lossy().to_string();
    log::info!("Transcript auto-saved to: {}", path_str);
    Ok(path_str)
}

/// Opens a directory in the system's file manager
/// Handles platform-specific path conversion (WSL Linux paths -> Windows UNC paths)
#[tauri::command]
pub async fn open_directory(path: String) -> Result<(), String> {
    use std::process::Command;

    let dir = std::path::Path::new(&path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Check if running under WSL
        let is_wsl = std::path::Path::new("/usr/lib/wsl/lib").exists();
        if is_wsl {
            // Convert Linux path to Windows UNC path via wslpath, then open with explorer.exe
            let wsl_path = Command::new("wslpath")
                .args(["-w", &path])
                .output()
                .map_err(|e| format!("Failed to convert WSL path: {}", e))?;

            if wsl_path.status.success() {
                let win_path = String::from_utf8_lossy(&wsl_path.stdout).trim().to_string();
                Command::new("explorer.exe")
                    .arg(&win_path)
                    .spawn()
                    .map_err(|e| format!("Failed to open directory in Explorer: {}", e))?;
            } else {
                // Fallback: explorer.exe can sometimes handle Linux paths directly
                Command::new("explorer.exe")
                    .arg(&path)
                    .spawn()
                    .map_err(|e| format!("Failed to open directory: {}", e))?;
            }
        } else {
            Command::new("xdg-open")
                .arg(&path)
                .spawn()
                .map_err(|e| format!("Failed to open directory: {}", e))?;
        }
    }

    Ok(())
}

/// Opens a native folder picker dialog
/// Returns the selected directory path, or None if cancelled
#[tauri::command]
pub async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let folder = app.dialog().file().blocking_pick_folder();

    match folder {
        Some(file_path) => {
            let path = file_path.as_path().ok_or("Invalid folder path")?;
            let path_str = path.to_string_lossy().to_string();
            log::info!("Directory picked: {}", path_str);
            Ok(Some(path_str))
        }
        None => {
            log::info!("Directory picker cancelled");
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_plain_filename() {
        assert!(validate_plain_filename("transcript.txt").is_ok());
        assert!(validate_plain_filename("2026-07-03 meeting notes.md").is_ok());
    }

    #[test]
    fn rejects_parent_traversal() {
        assert!(validate_plain_filename("../evil.txt").is_err());
        assert!(validate_plain_filename("..").is_err());
        assert!(validate_plain_filename("foo/../../evil.txt").is_err());
    }

    #[test]
    fn rejects_path_separators() {
        assert!(validate_plain_filename("sub/dir.txt").is_err());
        assert!(validate_plain_filename("sub\\dir.txt").is_err());
        assert!(validate_plain_filename("/etc/passwd").is_err());
        assert!(validate_plain_filename("C:\\Windows\\evil.txt").is_err());
    }

    #[test]
    fn rejects_empty_and_dot() {
        assert!(validate_plain_filename("").is_err());
        assert!(validate_plain_filename(".").is_err());
    }

    #[tokio::test]
    async fn save_transcript_rejects_traversal_filename() {
        let dir = std::env::temp_dir().join("echophrase-test-save-transcript");
        let _ = std::fs::create_dir_all(&dir);
        let result = save_transcript_to_directory(
            "content".to_string(),
            "../escaped.txt".to_string(),
            dir.to_string_lossy().to_string(),
        )
        .await;
        assert!(result.is_err());
        assert!(!dir.parent().unwrap().join("escaped.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
