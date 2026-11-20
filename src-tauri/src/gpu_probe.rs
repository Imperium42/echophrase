// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/// In-process NVIDIA GPU probing via NVML.
///
/// Replaces `Command::new("nvidia-smi")` calls. Spawning `nvidia-smi.exe` on
/// Windows can fail with `STATUS_DLL_INIT_FAILED` (0xC0000142) when the driver
/// install is in a bad state (mismatched `nvml.dll` / `nvapi64.dll` / etc.),
/// and the resulting Windows Error Reporting popup cannot be suppressed from
/// the parent's `Command` API. NVML's `Nvml::init()` exhibits the same failure
/// mode as a NULL handle inside our process, no popup.
///
/// Stub on macOS (NVIDIA does not ship NVML there) — all functions return
/// `None` / `false`.
#[cfg(any(target_os = "windows", target_os = "linux"))]
use std::sync::OnceLock;

#[cfg(any(target_os = "windows", target_os = "linux"))]
use nvml_wrapper::Nvml;

/// Cached `Nvml` handle. `None` when the driver is missing, mismatched, or no
/// NVIDIA GPU is present. Initialized at most once per process.
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn nvml() -> Option<&'static Nvml> {
    static CACHED: OnceLock<Option<Nvml>> = OnceLock::new();
    CACHED
        .get_or_init(|| match Nvml::init() {
            Ok(n) => Some(n),
            Err(e) => {
                log::info!(
                    "[gpu_probe] NVML init failed (no NVIDIA GPU or broken driver): {}",
                    e
                );
                None
            }
        })
        .as_ref()
}

/// Returns `true` when an NVIDIA driver is loadable and at least one GPU is
/// reported. Cheap (cached after first call). Use as a CUDA-presence probe in
/// place of the old `nvidia_smi_ok()` shell-out.
#[allow(dead_code)]
pub fn is_nvidia_present() -> bool {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        match nvml() {
            Some(n) => n.device_count().map(|c| c > 0).unwrap_or(false),
            None => false,
        }
    }
    #[cfg(target_os = "macos")]
    {
        false
    }
}

/// Returns `(name, vram_mb)` for GPU index 0, or `None` if no NVIDIA GPU is
/// reachable. Replaces the `nvidia-smi --query-gpu=name,memory.total` parse.
pub fn primary_gpu_info() -> Option<(String, u32)> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let nvml = nvml()?;
        let device = nvml.device_by_index(0).ok()?;
        let name = device.name().ok()?;
        let mem = device.memory_info().ok()?;
        // total is bytes; convert to MB the same way nvidia-smi --format=csv,nounits does.
        let vram_mb = (mem.total / 1024 / 1024) as u32;
        Some((name, vram_mb))
    }
    #[cfg(target_os = "macos")]
    {
        None
    }
}

/// Just the GPU name. Convenience wrapper for callsites that only need a label.
#[allow(dead_code)]
pub fn primary_gpu_name() -> Option<String> {
    primary_gpu_info().map(|(name, _)| name)
}
