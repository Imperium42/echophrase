// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Tray icon status indicator: idle / recording / transcribing.
//!
//! Owns the `TrayIcon` handle (otherwise dropped at end of `setup`) and exposes
//! a single `set` entry point that updates icon + tooltip. Three icon variants
//! are composited once at startup from the bundled `icon.png`: a stock copy for
//! Idle, plus red- and yellow-dot overlays for Recording / Transcribing.

use std::sync::Mutex;

use image::{ImageBuffer, Rgba, RgbaImage};
use tauri::{AppHandle, Manager, Wry, image::Image, menu::MenuItem, tray::TrayIcon};

/// Runtime tray status reflected in the tray icon and tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayStatus {
    /// No activity. Stock icon, "Echophrase (Idle)".
    Idle,
    /// Mic is capturing audio. Red dot overlay.
    Recording,
    /// Audio is being transcribed by a backend. Yellow dot overlay.
    Transcribing,
}

/// Managed state holding the live tray handle and pre-rendered icon variants.
struct TrayState {
    tray: TrayIcon<Wry>,
    /// Handle to the toggle-recording menu item so its label can be updated.
    toggle_item: MenuItem<Wry>,
    idle: Image<'static>,
    recording: Image<'static>,
    transcribing: Image<'static>,
    /// `None` until the first `set`, so the initial idle paint is never deduped away.
    current: Mutex<Option<TrayStatus>>,
}

const BASE_ICON_PNG: &[u8] = include_bytes!("../icons/32x32.png");

/// Install the tray status manager. Call once during `setup` after the
/// `TrayIcon` has been built. Decodes the bundled icon, composites the two
/// overlay variants, and stores everything in app state.
pub fn install(
    app: &AppHandle,
    tray: TrayIcon<Wry>,
    toggle_item: MenuItem<Wry>,
) -> anyhow::Result<()> {
    let base = image::load_from_memory(BASE_ICON_PNG)
        .map_err(|e| anyhow::anyhow!("decode tray base icon: {e}"))?
        .to_rgba8();

    // macOS Tahoe menu bar icons are template images: monochrome black+alpha
    // glyphs the system tints (white-ish on dark bars, dark on light/tinted) and
    // draws its own highlight/background behind. Derive the idle template from the
    // base icon's alpha so we keep a single source of truth. Recording/Transcribing
    // stay full-color (non-template) so their red/yellow dots remain vivid - the
    // template flag is toggled per-status in `set`.
    #[cfg(target_os = "macos")]
    let idle = to_tauri_image(to_template(&base));
    #[cfg(not(target_os = "macos"))]
    let idle = to_tauri_image(base.clone());
    let recording = to_tauri_image(with_dot(&base, [0xef, 0x44, 0x44, 0xff]));
    let transcribing = to_tauri_image(with_dot(&base, [0xea, 0xb3, 0x08, 0xff]));

    app.manage(TrayState {
        tray,
        toggle_item,
        idle,
        recording,
        transcribing,
        current: Mutex::new(None),
    });

    set(app, TrayStatus::Idle);
    Ok(())
}

/// Update the tray icon + tooltip to reflect the given status. No-op if
/// already in that state. Safe to call from any thread.
pub fn set(app: &AppHandle, status: TrayStatus) {
    let Some(state) = app.try_state::<TrayState>() else {
        log::debug!("tray_status::set called before install (status={status:?})");
        return;
    };

    {
        let mut current = state.current.lock().unwrap();
        if *current == Some(status) {
            return;
        }
        *current = Some(status);
    }

    let (icon, tooltip, menu_label) = match status {
        TrayStatus::Idle => (&state.idle, "Echophrase (Idle)", "Start Recording"),
        TrayStatus::Recording => (&state.recording, "Echophrase (Recording)", "Stop Recording"),
        TrayStatus::Transcribing => (
            &state.transcribing,
            "Echophrase (Transcribing)",
            "Stop Recording",
        ),
    };

    if let Err(e) = state.tray.set_icon(Some(icon.clone())) {
        log::warn!("tray set_icon failed: {e}");
    }
    // Only the monochrome idle icon is a macOS template image. The colored
    // recording/transcribing icons must NOT be templates or macOS flattens
    // their dots to monochrome.
    #[cfg(target_os = "macos")]
    if let Err(e) = state
        .tray
        .set_icon_as_template(matches!(status, TrayStatus::Idle))
    {
        log::warn!("tray set_icon_as_template failed: {e}");
    }
    if let Err(e) = state.tray.set_tooltip(Some(tooltip)) {
        log::warn!("tray set_tooltip failed: {e}");
    }
    if let Err(e) = state.toggle_item.set_text(menu_label) {
        log::warn!("tray menu set_text failed: {e}");
    }

    crate::overlay::on_status(app, status);
}

/// True while the tray reports a recording or a transcription in progress
/// (open-mic sessions stay `Recording` for their whole duration). False when
/// idle, or when no tray was installed. The model idle-unload watcher uses
/// this to avoid pulling the model out from under live work.
pub fn is_busy(app: &AppHandle) -> bool {
    app.try_state::<TrayState>()
        .map(|state| {
            matches!(
                *state.current.lock().unwrap(),
                Some(TrayStatus::Recording | TrayStatus::Transcribing)
            )
        })
        .unwrap_or(false)
}

/// RAII guard that flips the tray to `enter` on construction and `exit` on drop.
/// Useful for transient states like Transcribing where the exit must run on
/// every code path (Ok, early-return Err, panic-unwind).
pub struct StatusGuard {
    app: AppHandle,
    exit: TrayStatus,
}

impl StatusGuard {
    pub fn new(app: AppHandle, enter: TrayStatus, exit: TrayStatus) -> Self {
        set(&app, enter);
        Self { app, exit }
    }
}

impl Drop for StatusGuard {
    fn drop(&mut self) {
        set(&self.app, self.exit);
    }
}

/// Convert a color icon into a macOS template image: preserve the alpha mask
/// (which carries the glyph silhouette) and set every pixel's RGB to black.
/// macOS ignores the RGB of a template image and applies its own adaptive tint.
#[cfg(target_os = "macos")]
fn to_template(base: &RgbaImage) -> RgbaImage {
    let mut out = base.clone();
    for px in out.pixels_mut() {
        let a = px.0[3];
        *px = Rgba([0, 0, 0, a]);
    }
    out
}

fn with_dot(base: &RgbaImage, color: [u8; 4]) -> RgbaImage {
    let mut out = base.clone();
    let (w, h) = out.dimensions();
    // Dot occupies bottom-right ~45% of the icon. Slightly oversized for
    // legibility at 32x32 where every pixel counts.
    let radius = (w.min(h) as f32 * 0.28) as i32;
    let cx = w as i32 - radius - 1;
    let cy = h as i32 - radius - 1;

    for y in (cy - radius)..=(cy + radius) {
        for x in (cx - radius)..=(cx + radius) {
            if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
                continue;
            }
            let dx = x - cx;
            let dy = y - cy;
            let d2 = (dx * dx + dy * dy) as f32;
            // Soft 1px edge for anti-aliasing.
            let edge = (radius as f32 - d2.sqrt()).clamp(0.0, 1.0);
            if edge > 0.0 {
                let alpha = (color[3] as f32 * edge) as u8;
                out.put_pixel(
                    x as u32,
                    y as u32,
                    blend(
                        [color[0], color[1], color[2], alpha],
                        *out.get_pixel(x as u32, y as u32),
                    ),
                );
            }
        }
    }

    out
}

fn blend(src: [u8; 4], dst: Rgba<u8>) -> Rgba<u8> {
    let sa = src[3] as f32 / 255.0;
    let da = dst.0[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let mix = |s: u8, d: u8| -> u8 {
        let s = s as f32 * sa;
        let d = d as f32 * da * (1.0 - sa);
        ((s + d) / out_a).round().clamp(0.0, 255.0) as u8
    };
    Rgba([
        mix(src[0], dst.0[0]),
        mix(src[1], dst.0[1]),
        mix(src[2], dst.0[2]),
        (out_a * 255.0).round() as u8,
    ])
}

fn to_tauri_image(buf: ImageBuffer<Rgba<u8>, Vec<u8>>) -> Image<'static> {
    let (w, h) = buf.dimensions();
    Image::new_owned(buf.into_raw(), w, h)
}
