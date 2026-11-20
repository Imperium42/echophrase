// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! macOS TCC (Transparency, Consent, Control) microphone permission helpers.
//!
//! On modern macOS, simply enumerating CoreAudio input devices does NOT reliably
//! trigger the permission prompt — only `AVCaptureDevice.requestAccess(for: .audio)`
//! (or actually opening an `AudioUnit` input stream) does. Without an explicit
//! request, users on a fresh install see "Microphone not detected" forever with
//! no system dialog ever appearing.
//!
//! This module bridges to AVFoundation via objc2 + block2 to ask the OS for
//! permission and report the current status back to the frontend.
#![cfg(target_os = "macos")]

use block2::RcBlock;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::mpsc;
use std::time::Duration;

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeAudio: *const AnyObject;
}

/// AVAuthorizationStatus values from `<AVFoundation/AVCaptureDevice.h>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicAuthStatus {
    NotDetermined,
    Restricted,
    Denied,
    Authorized,
}

impl MicAuthStatus {
    fn from_raw(raw: i64) -> Self {
        match raw {
            3 => Self::Authorized,
            2 => Self::Denied,
            1 => Self::Restricted,
            _ => Self::NotDetermined,
        }
    }

    /// Stable lower-case string used by the frontend.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotDetermined => "not_determined",
            Self::Restricted => "restricted",
            Self::Denied => "denied",
            Self::Authorized => "authorized",
        }
    }
}

/// Read the current `AVAuthorizationStatus` for audio without prompting.
pub fn current_status() -> MicAuthStatus {
    // SAFETY: AVCaptureDevice is a system class loaded via the AVFoundation
    // framework link below; `authorizationStatusForMediaType:` is a class
    // method that takes an NSString and returns NSInteger (mapped to i64 on
    // aarch64). AVMediaTypeAudio is a global NSString constant exported by
    // AVFoundation. No state is mutated.
    unsafe {
        let cls = class!(AVCaptureDevice);
        let raw: i64 = msg_send![cls, authorizationStatusForMediaType: AVMediaTypeAudio];
        MicAuthStatus::from_raw(raw)
    }
}

/// Trigger the macOS permission prompt if the user has not yet decided,
/// blocking for up to 30 seconds for their response. If a decision was
/// already made (authorized / denied / restricted), returns immediately.
pub fn request_access_blocking() -> MicAuthStatus {
    let initial = current_status();
    if initial != MicAuthStatus::NotDetermined {
        return initial;
    }

    let (tx, rx) = mpsc::channel::<bool>();
    // Objective-C `BOOL` is `signed char` and must be marshaled through
    // objc2::runtime::Bool - the block2 trait bounds reject raw `bool`.
    let handler = RcBlock::new(move |granted: Bool| {
        let _ = tx.send(granted.as_bool());
    });

    // SAFETY: requestAccessForMediaType:completionHandler: is a class method
    // on AVCaptureDevice. The completion handler is retained by AVFoundation
    // for the duration of the async call; RcBlock manages the block lifetime.
    // We block here on a channel until the system either invokes the handler
    // or the 30s safety timeout fires.
    unsafe {
        let cls = class!(AVCaptureDevice);
        let _: () = msg_send![
            cls,
            requestAccessForMediaType: AVMediaTypeAudio,
            completionHandler: &*handler,
        ];
    }

    let _ = rx.recv_timeout(Duration::from_secs(30));
    current_status()
}

// ## Keystroke-posting permission (paste at cursor)
//
// macOS splits what System Settings shows as one "Accessibility" switch into
// three independent TCC services:
//
// | Service | Capability |
// |---|---|
// | `kTCCServiceAccessibility` | inspect and drive other apps' UI trees (AXUIElement) |
// | `kTCCServicePostEvent` | *send* synthetic input (`CGEventPost`) |
// | `kTCCServiceListenEvent` | *read* all input (`CGEventTap`) - i.e. keylogging |
//
// Pasting at the cursor needs only `kTCCServicePostEvent` - the send-only one.
// We deliberately query and request exactly that, via CoreGraphics'
// `CGPreflightPostEventAccess` / `CGRequestPostEventAccess`, rather than the
// broader `AXIsProcessTrusted` (which reports `kTCCServiceAccessibility`, a
// permission we never use). They are separate rows in `TCC.db`, so asking the
// wrong one can block a paste that would have worked fine.
//
// Echophrase never uses the AXUIElement or event-tap services at all: global
// hotkeys go through Carbon `RegisterEventHotKey`, which needs no permission.

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    /// `bool CGPreflightPostEventAccess(void)` (macOS 10.15+) - whether this
    /// process may post synthetic events. Never prompts, so we can ask before
    /// macOS gets a chance to show its context-free "control this computer"
    /// dialog and explain ourselves first.
    fn CGPreflightPostEventAccess() -> bool;

    /// `bool CGRequestPostEventAccess(void)` (macOS 10.15+) - raises the system
    /// permission dialog for post-event access and returns the resulting state.
    /// A no-op returning the current state once the user has decided.
    fn CGRequestPostEventAccess() -> bool;
}

/// Whether the app may synthesize the Cmd+V keystroke that powers "paste at
/// cursor" (`kTCCServicePostEvent`).
///
/// Purely a query - guaranteed not to prompt.
///
/// Caveat: CoreGraphics caches a `false` result for the life of the process. If
/// the user grants the permission while the app is running, this can keep
/// reporting `false` until Echophrase restarts - so treat a `false` here as
/// "not granted *yet in this process*", never as "the user refused".
pub fn can_post_keyboard_events() -> bool {
    // SAFETY: parameterless C function in the CoreGraphics framework returning
    // a C99 `bool`. Mutates no state and cannot fail.
    unsafe { CGPreflightPostEventAccess() }
}

/// Raise the system prompt for post-event access and report the result.
///
/// Prefer showing our own explanation first - this triggers the OS dialog whose
/// wording we cannot control.
pub fn request_post_event_access() -> bool {
    // SAFETY: same contract as CGPreflightPostEventAccess. May block briefly
    // while the system dialog is presented, so callers should keep it off the
    // async runtime's worker threads.
    unsafe { CGRequestPostEventAccess() }
}

/// An audio input device discovered via AVFoundation.
#[derive(Debug, Clone)]
pub struct AvAudioDevice {
    pub id: String,
    pub name: String,
}

/// Enumerate audio input devices via AVFoundation.
///
/// Avoids the cpal/CoreAudio HAL race where `host.input_devices()` returns an
/// empty iterator for a short window after TCC flips to Authorized. AVFoundation
/// reports devices the moment authorization status is `Authorized`, so the
/// "Microphone not detected" UX after a fresh grant goes away.
///
/// Uses the deprecated-but-still-functional `+[AVCaptureDevice
/// devicesWithMediaType:]` for compatibility with macOS 13.3+. The modern
/// `AVCaptureDeviceDiscoverySession` API requires per-version device-type
/// constants (`AVCaptureDeviceTypeMicrophone` is 14+ only) - revisit when the
/// minimum deployment target moves to macOS 14.
pub fn list_audio_input_devices() -> Vec<AvAudioDevice> {
    // SAFETY: AVCaptureDevice is loaded via the AVFoundation framework link
    // above. `devicesWithMediaType:` is a class method returning an autoreleased
    // NSArray of AVCaptureDevice objects. NSArray.count returns NSUInteger
    // (usize on 64-bit), objectAtIndex: returns an unretained id, and the
    // localizedName/uniqueID NSStrings are valid for the autorelease scope
    // (we copy them into owned Rust strings before returning).
    unsafe {
        let cls = class!(AVCaptureDevice);
        let devices: *mut AnyObject = msg_send![cls, devicesWithMediaType: AVMediaTypeAudio];
        if devices.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![devices, count];
        let mut out: Vec<AvAudioDevice> = Vec::with_capacity(count);
        for i in 0..count {
            let dev: *mut AnyObject = msg_send![devices, objectAtIndex: i];
            if dev.is_null() {
                continue;
            }
            let Some(name) = ns_string_to_owned(msg_send![dev, localizedName]) else {
                continue;
            };
            // Use localizedName for both id and name to stay compatible with
            // the cpal contract (settings persist a device by display name).
            // uniqueID would be more robust against duplicate-named devices
            // but would invalidate existing saved selections.
            out.push(AvAudioDevice {
                id: name.clone(),
                name,
            });
        }
        out
    }
}

/// Copy an NSString (as `*mut AnyObject`) into an owned Rust `String`.
///
/// Returns `None` if the pointer is null or `UTF8String` returns null.
///
/// # Safety
/// `ns_str` must be either null or a valid `NSString *` whose autorelease pool
/// outlives this call.
unsafe fn ns_string_to_owned(ns_str: *mut AnyObject) -> Option<String> {
    if ns_str.is_null() {
        return None;
    }
    let ptr: *const c_char = unsafe { msg_send![ns_str, UTF8String] };
    if ptr.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test - just confirms the AVFoundation bridge doesn't panic.
    /// On CI runners with no audio hardware the list may be empty, which is fine.
    #[test]
    fn list_audio_input_devices_does_not_panic() {
        let _devices = list_audio_input_devices();
    }
}
