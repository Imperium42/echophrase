// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Execution Provider abstraction for ONNX Runtime.
//!
//! Provides a unified interface for selecting GPU acceleration backends:
//! - DirectML (Windows - works on NVIDIA, AMD, Intel)
//! - CoreML (macOS via Apple Metal)
//! - CPU (fallback, and the only option on Linux/WSL)
//!
//! With ort's `load-dynamic` feature, ort loads the ORT shared library at
//! runtime and detects available devices via `Environment::devices()`.

use log::{info, warn};
#[cfg(target_os = "macos")]
use ort::execution_providers::CoreMLExecutionProvider;
#[cfg(target_os = "windows")]
use ort::execution_providers::DirectMLExecutionProvider;
use ort::execution_providers::ExecutionProviderDispatch;
use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// Types
// ============================================================================

/// Available execution providers for ONNX inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionProviderType {
    /// DirectML - Windows default, works on any GPU (NVIDIA, AMD, Intel)
    DirectMl,
    /// CoreML - macOS via Apple Metal
    CoreMl,
    /// CPU fallback - no GPU acceleration
    Cpu,
}

impl fmt::Display for ExecutionProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectMl => write!(f, "DirectML"),
            Self::CoreMl => write!(f, "CoreML"),
            Self::Cpu => write!(f, "CPU"),
        }
    }
}

#[allow(dead_code)]
impl ExecutionProviderType {
    /// Returns true if this is a GPU-accelerated provider
    pub fn is_gpu(&self) -> bool {
        !matches!(self, Self::Cpu)
    }

    /// Returns human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::DirectMl => "Windows GPU (any vendor)",
            Self::CoreMl => "Apple Silicon",
            Self::Cpu => "CPU (no GPU)",
        }
    }

    /// Convert to parakeet-rs ExecutionProvider enum.
    ///
    /// Returns None for CPU (parakeet-rs uses default) or unsupported providers.
    /// Note: Available variants depend on parakeet-rs feature flags:
    /// - Windows: DirectML only
    /// - Linux/macOS: CPU only
    pub fn to_parakeet_provider(&self) -> Option<parakeet_rs::ExecutionProvider> {
        match self {
            #[cfg(windows)]
            Self::DirectMl => Some(parakeet_rs::ExecutionProvider::DirectML),
            #[cfg(not(windows))]
            Self::DirectMl => None,
            Self::Cpu => None,    // parakeet-rs uses default for CPU
            Self::CoreMl => None, // Not supported by parakeet-rs
        }
    }
}

/// Status of an execution provider.
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionProviderStatus {
    pub provider: ExecutionProviderType,
    pub available: bool,
    pub reason: String,
    pub requires_download: bool,
    /// True when the user's hardware fundamentally cannot run this provider.
    /// Hard-incompatible providers should be HIDDEN in the UI, not shown disabled.
    pub hard_incompatible: bool,
}

/// Result of execution provider selection.
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionProviderSelection {
    /// The active execution provider
    pub active: ExecutionProviderType,
    /// User's preferred provider (may differ from active until restart)
    pub preferred: Option<ExecutionProviderType>,
    /// All available providers with status
    pub providers: Vec<ExecutionProviderStatus>,
    /// Human-readable status message
    pub status_message: String,
    /// True if running on CPU fallback (should show warning)
    pub is_cpu_fallback: bool,
}

/// Hardware device discovered by ort's `Environment::devices()`.
///
/// Used by the `list_ort_devices` Tauri command so the UI can show what
/// ort actually sees on the system (e.g. "Intel Arc - DirectML").
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    /// Execution provider name (e.g. "DmlExecutionProvider", "CPUExecutionProvider")
    pub ep_name: String,
    /// EP vendor (e.g. "NVIDIA", "Microsoft")
    pub ep_vendor: String,
    /// Hardware vendor / manufacturer
    pub hw_vendor: String,
    /// Device type: "CPU", "GPU", or "NPU"
    pub device_type: String,
    /// Hardware device ID
    pub device_id: u32,
}

// ============================================================================
// Platform Detection
// ============================================================================

/// Parse macOS version into (major, minor) tuple.
#[cfg(target_os = "macos")]
fn parse_macos_version() -> Option<(u32, u32)> {
    use std::sync::OnceLock;
    static CACHED: OnceLock<Option<(u32, u32)>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        let output = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()?;
        let version = String::from_utf8(output.stdout).ok()?;
        let parts: Vec<&str> = version.trim().split('.').collect();
        let major = parts.first()?.parse::<u32>().ok()?;
        let minor = parts
            .get(1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        Some((major, minor))
    })
}

/// Returns true on macOS 13.0 or later.
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub fn is_macos_13_or_later() -> bool {
    parse_macos_version()
        .map(|(major, _)| major >= 13)
        .unwrap_or(false)
}

#[allow(dead_code)]
#[cfg(not(target_os = "macos"))]
pub fn is_macos_13_or_later() -> bool {
    false
}

/// Returns true on macOS 13.3 or later (required by ORT 1.23+ for C++20 std::to_chars).
#[cfg(target_os = "macos")]
pub fn is_macos_13_3_or_later() -> bool {
    parse_macos_version()
        .map(|(major, minor)| major > 13 || (major == 13 && minor >= 3))
        .unwrap_or(false)
}

#[allow(dead_code)]
#[cfg(not(target_os = "macos"))]
pub fn is_macos_13_3_or_later() -> bool {
    false
}

/// Get the default execution provider for the current platform.
pub fn get_platform_default() -> ExecutionProviderType {
    #[cfg(target_os = "windows")]
    {
        ExecutionProviderType::DirectMl
    }
    #[cfg(target_os = "macos")]
    {
        if is_macos_13_or_later() {
            ExecutionProviderType::CoreMl
        } else {
            ExecutionProviderType::Cpu
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Linux/WSL: CPU only
        ExecutionProviderType::Cpu
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        ExecutionProviderType::Cpu
    }
}

// ============================================================================
// Device Enumeration (ort native)
// ============================================================================

/// Enumerate all devices ort can see on this system.
///
pub fn list_devices() -> Vec<DeviceInfo> {
    if !crate::is_onnx_runtime_available() {
        return Vec::new();
    }

    let env = match ort::environment::current() {
        Ok(env) => env,
        Err(e) => {
            warn!("ort::environment::current() failed: {} ({:?})", e, e);
            return Vec::new();
        }
    };

    let devices: Vec<DeviceInfo> = env
        .devices()
        .map(|d| {
            let ep_name = d.ep().unwrap_or("unknown").to_string();
            let ep_vendor = d.ep_vendor().unwrap_or("unknown").to_string();
            let hw_vendor = d.vendor().unwrap_or("unknown").to_string();
            let device_type = match d.ty() {
                ort::memory::DeviceType::CPU => "CPU",
                ort::memory::DeviceType::GPU => "GPU",
                ort::memory::DeviceType::NPU => "NPU",
            }
            .to_string();
            DeviceInfo {
                ep_name,
                ep_vendor,
                hw_vendor,
                device_type,
                device_id: d.id(),
            }
        })
        .collect();

    info!("ort enumerated {} device(s)", devices.len());
    for d in &devices {
        info!(
            "  ep={} ep_vendor={} hw_vendor={} type={} id={}",
            d.ep_name, d.ep_vendor, d.hw_vendor, d.device_type, d.device_id
        );
    }

    devices
}

// ============================================================================
// DirectML Verification (Windows only - kept for explicit user override path)
// ============================================================================

#[allow(dead_code)]
#[cfg(target_os = "windows")]
pub fn verify_directml_available() -> Result<bool, String> {
    let result = std::panic::catch_unwind(|| {
        let providers = [DirectMLExecutionProvider::default()
            .build()
            .error_on_failure()];
        !providers.is_empty()
    });

    match result {
        Ok(true) => {
            log::info!("DirectML verification: EP can be registered");
            Ok(true)
        }
        Ok(false) => {
            log::warn!("DirectML verification: EP list was empty");
            Err("DirectML provider list empty".to_string())
        }
        Err(e) => {
            log::error!("DirectML verification failed: {:?}", e);
            Err(format!("DirectML not available: {:?}", e))
        }
    }
}

#[allow(dead_code)]
#[cfg(not(target_os = "windows"))]
pub fn verify_directml_available() -> Result<bool, String> {
    Err("DirectML only available on Windows".to_string())
}

// ============================================================================
// Execution Provider Building (manual override path)
// ============================================================================

/// Build the explicit execution provider list when the user has forced a
/// specific EP via Settings > GPU Acceleration.
///
/// When the user has no preference, callers should use
/// `SessionBuilder::with_auto_device(AutoDevicePolicy::MaxPerformance)` instead -
/// it lets ort pick the best device natively (DirectML, CoreML, NPU, CPU).
pub fn build_execution_providers(
    preferred: Option<ExecutionProviderType>,
) -> Vec<ExecutionProviderDispatch> {
    #[allow(unused_mut)]
    let mut providers: Vec<ExecutionProviderDispatch> = Vec::new();
    let preferred = preferred.unwrap_or_else(get_platform_default);

    log::info!("Building explicit execution providers for: {:?}", preferred);

    match preferred {
        ExecutionProviderType::DirectMl => {
            #[cfg(target_os = "windows")]
            providers.push(DirectMLExecutionProvider::default().build());
            #[cfg(not(target_os = "windows"))]
            log::warn!("DirectML requested but not available on this platform");
        }
        ExecutionProviderType::CoreMl => {
            #[cfg(target_os = "macos")]
            {
                if is_macos_13_3_or_later() {
                    providers.push(CoreMLExecutionProvider::default().build());
                } else {
                    warn!("CoreML unavailable - ONNX Runtime requires macOS 13.3+");
                }
            }
            #[cfg(not(target_os = "macos"))]
            log::warn!("CoreML requested but not available on this platform");
        }
        ExecutionProviderType::Cpu => {
            // No EPs - CPU is the implicit default
        }
    }

    providers
}

// ============================================================================
// Status Reporting
// ============================================================================

/// Parse execution provider string to type.
///
/// Unrecognized values (including the retired `"cuda"` that older settings
/// files may still contain) return `None`, which callers treat as auto-detect.
pub fn parse_execution_provider(s: &str) -> Option<ExecutionProviderType> {
    match s.to_lowercase().as_str() {
        "directml" | "direct_ml" => Some(ExecutionProviderType::DirectMl),
        "coreml" | "core_ml" => Some(ExecutionProviderType::CoreMl),
        "cpu" => Some(ExecutionProviderType::Cpu),
        _ => None,
    }
}

/// Get status of all execution providers for UI display.
///
/// Built from compiled-in platform knowledge only; no ORT calls.
#[allow(clippy::vec_init_then_push)]
pub fn get_execution_provider_status(preferred_str: Option<&str>) -> ExecutionProviderSelection {
    let platform_default = get_platform_default();
    let preferred_type = preferred_str.and_then(parse_execution_provider);
    let active_preferred = preferred_type.unwrap_or(platform_default);

    // Build provider list from static/compiled-in knowledge. Does NOT call into
    // ORT (no list_devices, no ort::environment::current) so this is instant.
    // ORT device enumeration is only needed for the Settings detail view
    // (list_ort_devices command), not for determining which EP is active.

    let mut providers = Vec::new();

    // DirectML (Windows only - always available on Windows 10+)
    #[cfg(target_os = "windows")]
    providers.push(ExecutionProviderStatus {
        provider: ExecutionProviderType::DirectMl,
        available: true,
        reason: "Built-in Windows GPU support".to_string(),
        requires_download: false,
        hard_incompatible: false,
    });

    // CoreML (macOS only - available if macOS 13.3+)
    #[cfg(target_os = "macos")]
    providers.push(ExecutionProviderStatus {
        provider: ExecutionProviderType::CoreMl,
        available: is_macos_13_3_or_later(),
        reason: if is_macos_13_3_or_later() {
            "Apple Metal GPU acceleration".to_string()
        } else {
            "Requires macOS 13.3+".to_string()
        },
        requires_download: false,
        hard_incompatible: false,
    });

    // CPU (always available)
    providers.push(ExecutionProviderStatus {
        provider: ExecutionProviderType::Cpu,
        available: true,
        reason: "Very slow fallback for potatoes".to_string(),
        requires_download: false,
        hard_incompatible: false,
    });

    let active = determine_active_provider(active_preferred, &providers);
    let is_cpu_fallback =
        active == ExecutionProviderType::Cpu && active_preferred != ExecutionProviderType::Cpu;

    let status_message = if is_cpu_fallback {
        "Running on CPU (no GPU acceleration available)".to_string()
    } else {
        match active {
            ExecutionProviderType::DirectMl => "Using DirectML (Windows GPU)".to_string(),
            ExecutionProviderType::CoreMl => "Using CoreML (Apple GPU)".to_string(),
            ExecutionProviderType::Cpu => "Using CPU".to_string(),
        }
    };

    ExecutionProviderSelection {
        active,
        preferred: preferred_type,
        providers,
        status_message,
        is_cpu_fallback,
    }
}

/// Determine which provider will actually be used based on preference and availability.
///
/// Fallback: preferred -> platform default (DirectML/CoreML) -> CPU
fn determine_active_provider(
    preferred: ExecutionProviderType,
    providers: &[ExecutionProviderStatus],
) -> ExecutionProviderType {
    if let Some(status) = providers.iter().find(|p| p.provider == preferred) {
        if status.available {
            return preferred;
        }
    }

    let platform_default = get_platform_default();
    if platform_default != preferred {
        if let Some(status) = providers.iter().find(|p| p.provider == platform_default) {
            if status.available {
                return platform_default;
            }
        }
    }

    ExecutionProviderType::Cpu
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_default_windows() {
        #[cfg(target_os = "windows")]
        assert_eq!(get_platform_default(), ExecutionProviderType::DirectMl);
    }

    #[test]
    fn test_platform_default_linux() {
        #[cfg(target_os = "linux")]
        assert_eq!(get_platform_default(), ExecutionProviderType::Cpu);
    }

    #[test]
    fn test_execution_provider_display() {
        assert_eq!(ExecutionProviderType::DirectMl.to_string(), "DirectML");
        assert_eq!(ExecutionProviderType::Cpu.to_string(), "CPU");
    }

    #[test]
    fn test_execution_provider_serde() {
        let ep = ExecutionProviderType::DirectMl;
        let json = serde_json::to_string(&ep).unwrap();
        assert_eq!(json, "\"direct_ml\"");

        let parsed: ExecutionProviderType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ep);
    }

    #[test]
    fn test_execution_provider_is_gpu() {
        assert!(ExecutionProviderType::DirectMl.is_gpu());
        assert!(ExecutionProviderType::CoreMl.is_gpu());
        assert!(!ExecutionProviderType::Cpu.is_gpu());
    }

    #[test]
    fn test_get_status_includes_cpu() {
        let status = get_execution_provider_status(None);
        assert!(
            status
                .providers
                .iter()
                .any(|p| p.provider == ExecutionProviderType::Cpu && p.available)
        );
    }

    #[test]
    fn test_build_providers_cpu_mode() {
        let providers = build_execution_providers(Some(ExecutionProviderType::Cpu));
        assert!(providers.is_empty());
    }

    #[test]
    fn test_parse_retired_cuda_is_auto() {
        // Old settings files may still hold "cuda"; it must degrade to auto-detect.
        assert_eq!(parse_execution_provider("cuda"), None);
    }
}
