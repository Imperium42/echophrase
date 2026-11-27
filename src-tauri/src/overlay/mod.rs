// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Floating recording overlay: a small pill (bottom-center by default, see
//! `recording_overlay_position`) on the monitor under the cursor, showing a
//! live voice waveform and a cancel X.
//!
//! Driven entirely by `tray_status::set` (the single recording-state choke
//! point) plus `emit_levels` from the capture loops, so the main window's
//! state machine stays the only source of truth.
//!
//! Window recipe credit: the undecorated, transparent, always-on-top,
//! non-focusable, cursor-monitor work-area placement is borrowed from Handy
//! (<https://github.com/cjpais/Handy>, `src-tauri/src/overlay.rs`, MIT License,
//! Copyright (c) CJ Pais).
//!
//! macOS: TODO - convert to a non-activating NSPanel (tauri-nspanel) so it
//! floats over fullscreen apps and can never take key focus. Until then it is
//! a plain always-on-top window there (unverified on macOS).

pub mod levels;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::tray_status::TrayStatus;

pub const OVERLAY_LABEL: &str = "recording_overlay";
/// Window size in logical px. Larger than the pill so its shadow has room.
const WIDTH: f64 = 268.0;
const HEIGHT: f64 = 72.0;
/// Gap between the window and the work-area edges it is anchored to.
const EDGE_MARGIN: f64 = 18.0;
/// Must cover the frontend's exit animation before the window hides.
const HIDE_DELAY: Duration = Duration::from_millis(240);

/// True while the overlay is (or is about to be) on screen. Lock-free fast
/// path for the capture loops; `OVERLAY` is the source of truth.
static VISIBLE: AtomicBool = AtomicBool::new(false);
/// Serializes every show/hide transition, including the delayed hide.
static OVERLAY: Mutex<Overlay> = Mutex::new(Overlay {
    state: OverlayState::Hidden,
    generation: 0,
});

struct Overlay {
    state: OverlayState,
    /// Bumped on every transition so a stale delayed hide never wins.
    generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OverlayState {
    Hidden,
    Recording,
    Transcribing,
}

/// React to a recording-state change. Called from `tray_status::set` while it
/// holds its own lock, so transitions arrive in order.
pub fn on_status(app: &AppHandle, status: TrayStatus) {
    let enabled = crate::settings::AppSettings::get().recording_overlay_enabled;
    let mut overlay = OVERLAY.lock().unwrap();
    let next = match status {
        TrayStatus::Recording if enabled => OverlayState::Recording,
        // Only a visible pill switches to "transcribing"; turning the setting
        // off mid open-mic session hides it at the next transition.
        TrayStatus::Transcribing if enabled && overlay.state != OverlayState::Hidden => {
            OverlayState::Transcribing
        }
        _ => OverlayState::Hidden,
    };
    if next == overlay.state {
        return;
    }
    let was_hidden = overlay.state == OverlayState::Hidden;
    overlay.state = next;
    overlay.generation += 1;
    VISIBLE.store(next != OverlayState::Hidden, Ordering::Release);
    let _ = app.emit_to(OVERLAY_LABEL, "overlay-state", next);

    if next == OverlayState::Hidden {
        schedule_hide(app, overlay.generation);
    } else if was_hidden {
        // Place only when appearing: open mic flips Recording/Transcribing per
        // utterance and the pill must not chase the cursor between monitors.
        show(app);
    }
}

/// True while the pill is on screen; lets the capture loops skip the FFT.
pub fn is_visible() -> bool {
    VISIBLE.load(Ordering::Acquire)
}

/// Forward one waveform frame to the overlay. Cheap no-op while hidden.
pub fn emit_levels(app: &AppHandle, bands: &[f32; levels::BAND_COUNT]) {
    if is_visible() {
        let _ = app.emit_to(OVERLAY_LABEL, "overlay-levels", bands);
    }
}

fn show(app: &AppHandle) {
    let Some(window) = ensure_window(app) else {
        return;
    };
    place_on_cursor_monitor(app, &window);
    platform_show(&window);
    // Re-assert topmost: Windows can drop it after the taskbar takes focus.
    let _ = window.set_always_on_top(true);
}

/// Hide after the page's exit animation, unless something newer happened.
fn schedule_hide(app: &AppHandle, generation: u64) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(HIDE_DELAY).await;
        let overlay = OVERLAY.lock().unwrap();
        if overlay.generation != generation {
            return; // re-shown (or changed) meanwhile
        }
        if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
            platform_hide(&window);
        }
    });
}

/// Windows: show without activating. tao only honors `focused(false)` on the
/// first show, so later `show()` calls use SW_SHOW and could take focus from
/// the app the user is dictating into. Visibility is driven by Win32 alone
/// (show and hide), so tao's cached visible flag is never consulted.
#[cfg(windows)]
fn platform_show(window: &WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOWNOACTIVATE, ShowWindow};
    match window.hwnd() {
        // SAFETY: the HWND belongs to a live window owned by this process.
        Ok(hwnd) => unsafe {
            ShowWindow(hwnd.0 as _, SW_SHOWNOACTIVATE);
        },
        Err(e) => log::warn!("[overlay] no HWND to show: {e}"),
    }
}

#[cfg(windows)]
fn platform_hide(window: &WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SW_HIDE, ShowWindow};
    if let Ok(hwnd) = window.hwnd() {
        // SAFETY: the HWND belongs to a live window owned by this process.
        unsafe {
            ShowWindow(hwnd.0 as _, SW_HIDE);
        }
    }
}

#[cfg(not(windows))]
fn platform_show(window: &WebviewWindow) {
    if let Err(e) = window.show() {
        log::warn!("[overlay] show failed: {e}");
    }
}

#[cfg(not(windows))]
fn platform_hide(window: &WebviewWindow) {
    let _ = window.hide();
}

/// Get the overlay window, creating it hidden on first use.
fn ensure_window(app: &AppHandle) -> Option<WebviewWindow> {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        return Some(window);
    }
    let (width, height) = logical_size();
    let built = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay".into()))
        .title("Echophrase Recording")
        .inner_size(width, height)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .visible(false)
        .build();
    match built {
        Ok(window) => Some(window),
        Err(e) => {
            log::warn!("[overlay] could not create window: {e}");
            None
        }
    }
}

/// Anchor (per the `recording_overlay_position` setting) inside the work area
/// (excludes the taskbar/Dock) of the monitor under the cursor, falling back
/// to the primary monitor.
fn place_on_cursor_monitor(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let (width, height) = logical_size();
    let area = monitor.work_area();
    let w = (width * scale).round() as i32;
    let h = (height * scale).round() as i32;
    let margin = (EDGE_MARGIN * scale) as i32;
    let position = crate::settings::AppSettings::get().recording_overlay_position;
    let (vertical, horizontal) = position.split_once('-').unwrap_or(("bottom", "center"));
    let x = match horizontal {
        "left" => area.position.x + margin,
        "right" => area.position.x + area.size.width as i32 - w - margin,
        _ => area.position.x + (area.size.width as i32 - w) / 2,
    };
    let y = match vertical {
        "top" => area.position.y + margin,
        _ => area.position.y + area.size.height as i32 - h - margin,
    };
    // Move first, then fix the size: crossing to a monitor with another DPI
    // rescales the window, so the logical size is re-applied afterwards.
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(LogicalSize::new(width, height));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

/// Window size grown by the OS text scale: WebView2 applies Windows'
/// "Text size" accessibility setting as a page zoom, so a fixed size would clip
/// the pill (seen at 125%). Same fix as Handy's overlay.
fn logical_size() -> (f64, f64) {
    let text_scale = text_scale_factor();
    (WIDTH * text_scale, HEIGHT * text_scale)
}

/// Settings > Accessibility > Text size, as a factor (1.0-2.25).
#[cfg(windows)]
fn text_scale_factor() -> f64 {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    let key: Vec<u16> = "Software\\Microsoft\\Accessibility\0"
        .encode_utf16()
        .collect();
    let name: Vec<u16> = "TextScaleFactor\0".encode_utf16().collect();
    let mut percent: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: both names are NUL-terminated UTF-16 buffers that outlive the
    // call, and the output buffer is a u32 whose byte size is passed in `size`.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut percent as *mut u32).cast(),
            &mut size,
        )
    };
    if status == 0 {
        f64::from(percent.clamp(100, 225)) / 100.0
    } else {
        1.0 // value absent = 100%
    }
}

#[cfg(not(windows))]
fn text_scale_factor() -> f64 {
    1.0
}

/// Current overlay state, for a page that loaded after the last emit.
#[tauri::command]
pub fn overlay_get_state() -> OverlayState {
    OVERLAY.lock().unwrap().state
}

/// The overlay's X: ask the main window to cancel (it owns the recording
/// state machine and already knows how to discard every session type).
#[tauri::command]
pub fn overlay_cancel(app: AppHandle) {
    let _ = app.emit_to("main", "overlay-cancel", ());
}
