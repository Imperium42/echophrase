// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Shared WSL / WSLg detection helpers.
//!
//! WSLg exposes the Windows microphone as a **PulseAudio** source (`RDPSource`)
//! over a fixed server socket - not as an ALSA device. Several call sites
//! (device enumeration, capture, the `PULSE_SERVER` bootstrap in `main`) need
//! the same three facts, so they live here instead of being re-inlined.

/// WSLg's fixed PulseAudio server socket. `parec`/`pactl` talk to the Windows
/// host audio stack through this when `PULSE_SERVER` points at it.
pub const WSLG_PULSE_SERVER: &str = "/mnt/wslg/PulseServer";

/// PulseAudio source name WSLg publishes for the passed-through Windows mic.
pub const RDP_SOURCE: &str = "RDPSource";

/// True when running inside a WSL distro (env markers set by the WSL runtime).
///
/// Cheap env-only check - does not touch the filesystem. Use
/// [`wslg_pulse_available`] when you specifically need the audio bridge.
pub fn is_wsl() -> bool {
    std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some()
}

/// True when WSLg's PulseAudio bridge is actually reachable: a WSL distro
/// **and** the [`WSLG_PULSE_SERVER`] socket exists. This is the precise gate
/// for routing capture through `parec` instead of cpal's ALSA-only host.
pub fn wslg_pulse_available() -> bool {
    is_wsl() && std::path::Path::new(WSLG_PULSE_SERVER).exists()
}
