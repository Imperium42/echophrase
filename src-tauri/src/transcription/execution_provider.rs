// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Execution Provider abstraction for ONNX Runtime.
//!
//! Provides a unified interface for selecting GPU acceleration backends:
//! - DirectML (Windows default - works on NVIDIA, AMD, Intel)
//! - CUDA (NVIDIA only, fastest)
//! - CoreML (macOS via Apple Metal)
//! - CPU (fallback)
//!
//! Since ort 2.0.0-rc.12 with `load-dynamic`, ort loads the ORT shared
//! library at runtime and detects available devices via `Environment::devices()`.
//! The hand-rolled scanning of `/usr/local/cuda` / `Program Files\NVIDIA`
//! was deleted - it duplicated what ort does natively.

use log::{info, warn};
#[cfg(feature = "cuda")]
use ort::execution_providers::CUDAExecutionProvider;
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
    /// CUDA - NVIDIA GPUs only
    Cuda,
    /// CoreML - macOS via Apple Metal
    CoreMl,
    /// CPU fallback - no GPU acceleration
    Cpu,
}

impl fmt::Display for ExecutionProviderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectMl => write!(f, "DirectML"),
            Self::Cuda => write!(f, "CUDA"),
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
            Self::Cuda => "NVIDIA CUDA (fastest)",
            Self::CoreMl => "Apple Silicon",
            Self::Cpu => "CPU (no GPU)",
        }
    }

    /// Convert to parakeet-rs ExecutionProvider enum.
    ///
    /// Returns None for CPU (parakeet-rs uses default) or unsupported providers.
    /// Note: Available variants depend on parakeet-rs feature flags:
    /// - Windows: DirectML only (cuda feature not enabled)
    /// - Linux/macOS: CPU only
    pub fn to_parakeet_provider(&self) -> Option<parakeet_rs::ExecutionProvider> {
        match self {
            #[cfg(windows)]
            Self::DirectMl => Some(parakeet_rs::ExecutionProvider::DirectML),
            #[cfg(not(windows))]
            Self::DirectMl => None,
            Self::Cuda => None, // ORT handles CUDA directly, not via parakeet-rs
            Self::Cpu => None,  // parakeet-rs uses default for CPU
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
    /// True when the user's hardware fundamentally cannot run this provider
    /// (eg CUDA on a machine with no NVIDIA GPU). Hard-incompatible providers
    /// should be HIDDEN in the UI, not shown disabled -- there's nothing the
    /// user can install to fix it. Soft-incompatible (eg NVIDIA GPU present
    /// but CUDA/cuDNN missing) stays visible-but-disabled so the user knows
    /// what to install.
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
/// ort actually sees on the system (e.g. "NVIDIA GeForce RTX 3090 - CUDA").
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    /// Execution provider name (e.g. "CUDAExecutionProvider", "CPUExecutionProvider")
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
        // Linux: CPU default; ort auto-device picks CUDA if hardware is present
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

/// Log CUDA-relevant environment state once per process.
///
/// Ort's bundled CUDA EP fails silently when it cannot `dlopen` its provider
/// library (missing cuDNN, wrong CUDA major, Blackwell gencode not in the
/// prebuilt, etc.) - `Environment::devices()` just omits the CUDA entry and
/// we get a bogus "incompatible" badge. This helper surfaces the actual
/// environment state so we can tell which of those it is. Runs at most once
/// to avoid log spam.
#[cfg(feature = "cuda")]
fn log_cuda_env_diagnostics_once() {
    use std::sync::OnceLock;
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let ort_cuda_version =
            std::env::var("ORT_CUDA_VERSION").unwrap_or_else(|_| "<unset>".to_string());
        info!("[CUDA diag] ORT_CUDA_VERSION={}", ort_cuda_version);

        #[cfg(target_os = "linux")]
        {
            let ld_library_path = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
            let cuda_entries: Vec<&str> = ld_library_path
                .split(':')
                .filter(|p| {
                    let lower = p.to_lowercase();
                    lower.contains("cuda") || lower.contains("cudnn")
                })
                .collect();
            info!(
                "[CUDA diag] LD_LIBRARY_PATH cuda/cudnn entries ({}): {:?}",
                cuda_entries.len(),
                cuda_entries
            );

            // `ldconfig -p` is a read-only query of the system's dynamic
            // linker cache; it does not modify anything. We filter for
            // libcudart / libcudnn so the log stays focused.
            match std::process::Command::new("ldconfig").arg("-p").output() {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let matches: Vec<&str> = stdout
                        .lines()
                        .filter(|l| {
                            let lower = l.to_lowercase();
                            lower.contains("libcudart") || lower.contains("libcudnn")
                        })
                        .collect();
                    info!(
                        "[CUDA diag] ldconfig libcudart/libcudnn matches: {}",
                        matches.len()
                    );
                    for m in matches.iter().take(20) {
                        info!("[CUDA diag]   {}", m.trim());
                    }
                }
                Ok(out) => warn!(
                    "[CUDA diag] ldconfig -p exited {}: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
                Err(e) => warn!("[CUDA diag] ldconfig -p failed to spawn: {}", e),
            }
        }

        #[cfg(target_os = "windows")]
        {
            let path_var = std::env::var("PATH").unwrap_or_default();
            let cuda_entries: Vec<&str> = path_var
                .split(';')
                .filter(|p| {
                    let lower = p.to_lowercase();
                    lower.contains("cuda") || lower.contains("cudnn")
                })
                .collect();
            info!(
                "[CUDA diag] PATH cuda/cudnn entries ({}): {:?}",
                cuda_entries.len(),
                cuda_entries
            );
        }
    });
}

/// Register the CUDA EP shared library with ort's environment so that
/// `devices()` can enumerate CUDA GPUs. Without this, `GetEpDevices` only
/// returns the CPU provider because ORT hasn't loaded the CUDA EP yet.
///
/// The `.virtual` suffix on the name tells ORT to create virtual device
/// entries for enumeration (required for `devices()` to see CUDA GPUs).
///
/// Runs at most once per process -- subsequent calls are no-ops.
#[cfg(feature = "cuda")]
fn register_cuda_ep_once(env: &std::sync::Arc<ort::environment::Environment>) {
    use std::sync::OnceLock;
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        // The CUDA EP .so is placed next to the binary by ort's copy-dylibs feature.
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));

        if let Some(dir) = exe_dir {
            #[cfg(target_os = "linux")]
            let cuda_lib = dir.join("libonnxruntime_providers_cuda.so");
            #[cfg(target_os = "windows")]
            let cuda_lib = dir.join("onnxruntime_providers_cuda.dll");

            if cuda_lib.exists() {
                match env.register_ep_library("CUDA.virtual", &cuda_lib) {
                    Ok(_handle) => {
                        info!("[CUDA diag] Registered CUDA EP library from {:?}", cuda_lib);
                    }
                    Err(e) => {
                        warn!(
                            "[CUDA diag] Failed to register CUDA EP library {:?}: {} ({:?})",
                            cuda_lib, e, e
                        );
                    }
                }
            } else {
                warn!(
                    "[CUDA diag] CUDA EP library not found at {:?} -- copy-dylibs may not have placed it",
                    cuda_lib
                );
            }
        }
    });
}

/// Enumerate all devices ort can see on this system.
///
/// Replaces the hand-rolled `find_cuda_installation()` /
/// `find_cuda_dll_in_dir()` / `find_cudnn_in_cuda_dir()` helpers from before
/// the rc.12 upgrade. Now ort tells us natively what's available.
pub fn list_devices() -> Vec<DeviceInfo> {
    if !crate::is_onnx_runtime_available() {
        return Vec::new();
    }

    #[cfg(feature = "cuda")]
    log_cuda_env_diagnostics_once();

    let env = match ort::environment::current() {
        Ok(env) => env,
        Err(e) => {
            warn!("ort::environment::current() failed: {} ({:?})", e, e);
            return Vec::new();
        }
    };

    #[cfg(feature = "cuda")]
    register_cuda_ep_once(&env);

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

/// Tri-state CUDA availability. Distinguishes "no NVIDIA GPU at all" (hard
/// incompat, hide the option) from "GPU present but libraries missing" (soft
/// incompat, show disabled with install hint).
///
/// - `HardIncompat`: `nvidia-smi` failed -- no NVIDIA GPU. Nothing the user can
///   install will fix it. UI should HIDE the CUDA option entirely.
/// - `SoftIncompat`: NVIDIA GPU present but CUDA EP library missing. UI should
///   show CUDA as disabled with a "Download CUDA/cuDNN" hint.
/// - `Available`: `nvidia-smi` OK + CUDA EP library present. Good to go.
#[cfg(feature = "cuda")]
#[derive(Debug, Clone)]
pub enum CudaAvailability {
    Available,
    SoftIncompat,
    HardIncompat,
}

#[cfg(feature = "cuda")]
impl CudaAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
    pub fn is_hard_incompat(&self) -> bool {
        matches!(self, Self::HardIncompat)
    }
}

/// In-process NVIDIA-GPU-present check via NVML. Returns true when the driver
/// is loadable AND at least one GPU is reported. Replaces the previous
/// `nvidia-smi` shell-out and `nvml.dll` `LoadLibraryExW` pre-flight.
///
/// Why: spawning `nvidia-smi.exe` on Windows can fail with
/// `STATUS_DLL_INIT_FAILED` (0xC0000142) when *any* of its DLL deps
/// (`nvml.dll`, `nvapi64.dll`, `cudart64_*.dll`, ...) fail to initialize after
/// a partial driver update. The previous fix only checked `nvml.dll`, missing
/// the other failure paths. Calling NVML in-process via `nvml-wrapper` has the
/// same failure mode as our old probe (Err on init, no popup) but covers the
/// real cases and avoids the child-process WER dialog entirely.
///
/// Result is cached by `gpu_probe::is_nvidia_present()` (one syscall per
/// process).
#[cfg(feature = "cuda")]
pub(crate) fn nvidia_smi_ok() -> bool {
    crate::gpu_probe::is_nvidia_present()
}

#[cfg(feature = "cuda")]
fn probe_cuda_available() -> CudaAvailability {
    use std::sync::OnceLock;
    static RESULT: OnceLock<CudaAvailability> = OnceLock::new();
    RESULT
        .get_or_init(|| {
            // Check 1: nvidia-smi succeeds (driver + GPU present).
            // If this fails, the user has no NVIDIA GPU -- hard incompatible.
            let smi_ok = nvidia_smi_ok();

            if !smi_ok {
                info!("[CUDA probe] nvidia-smi failed or no GPU found (hard incompat)");
                return CudaAvailability::HardIncompat;
            }

            // Check 2: CUDA EP .so exists next to binary (placed by ort's copy-dylibs).
            // Soft incompat -- user has NVIDIA GPU but missing CUDA libraries.
            let lib_exists = std::env::current_exe()
                .ok()
                .and_then(|p| {
                    p.parent().map(|d| {
                        #[cfg(target_os = "linux")]
                        let lib = d.join("libonnxruntime_providers_cuda.so");
                        #[cfg(target_os = "windows")]
                        let lib = d.join("onnxruntime_providers_cuda.dll");
                        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
                        let lib = d.join("nonexistent");
                        lib.exists()
                    })
                })
                .unwrap_or(false);

            if !lib_exists {
                info!("[CUDA probe] CUDA EP library not found next to binary (soft incompat)");
                return CudaAvailability::SoftIncompat;
            }

            info!("[CUDA probe] CUDA available (nvidia-smi OK + EP library present)");
            CudaAvailability::Available
        })
        .clone()
}

/// Actionable hint shown in the Settings UI when ort did not report a CUDA
/// device. Platform-specific so users know where to look first.
#[cfg(all(feature = "cuda", target_os = "linux"))]
fn cuda_unavailable_hint() -> String {
    "CUDA not detected. Install CUDA Toolkit 13.0+ and cuDNN 9.19+, ensure both are on \
     LD_LIBRARY_PATH, and restart. On Ubuntu/WSL: sudo apt install cuda-toolkit-13-0 libcudnn9-cuda-13"
        .to_string()
}

#[cfg(all(feature = "cuda", target_os = "windows"))]
fn cuda_unavailable_hint() -> String {
    "CUDA not detected. Install CUDA Toolkit 13.0+ and cuDNN 9.19+, ensure both are on PATH, \
     and restart. DirectML is used as a fallback on Windows."
        .to_string()
}

#[cfg(all(feature = "cuda", not(any(target_os = "linux", target_os = "windows"))))]
fn cuda_unavailable_hint() -> String {
    "No CUDA-capable device detected by ONNX Runtime".to_string()
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
/// it lets ort pick the best device natively (CUDA, DirectML, CoreML, NPU, CPU).
pub fn build_execution_providers(
    preferred: Option<ExecutionProviderType>,
) -> Vec<ExecutionProviderDispatch> {
    #[allow(unused_mut)]
    let mut providers: Vec<ExecutionProviderDispatch> = Vec::new();
    let preferred = preferred.unwrap_or_else(get_platform_default);

    log::info!("Building explicit execution providers for: {:?}", preferred);

    match preferred {
        ExecutionProviderType::Cuda => {
            #[cfg(feature = "cuda")]
            providers.push(CUDAExecutionProvider::default().build());
            #[cfg(not(feature = "cuda"))]
            log::warn!("CUDA requested but cuda feature is disabled at compile time");
            #[cfg(target_os = "windows")]
            providers.push(DirectMLExecutionProvider::default().build());
        }
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
pub fn parse_execution_provider(s: &str) -> Option<ExecutionProviderType> {
    match s.to_lowercase().as_str() {
        "directml" | "direct_ml" => Some(ExecutionProviderType::DirectMl),
        "cuda" => Some(ExecutionProviderType::Cuda),
        "coreml" | "core_ml" => Some(ExecutionProviderType::CoreMl),
        "cpu" => Some(ExecutionProviderType::Cpu),
        _ => None,
    }
}

/// Get status of all execution providers for UI display.
///
/// Uses ort's native `Environment::devices()` to determine availability
/// instead of scanning `/usr/local/cuda` and friends. CUDA is reported as
/// available if ort enumerates a `CUDAExecutionProvider` device.
#[allow(clippy::vec_init_then_push)]
pub fn get_execution_provider_status(preferred_str: Option<&str>) -> ExecutionProviderSelection {
    let platform_default = get_platform_default();
    let preferred_type = preferred_str.and_then(parse_execution_provider);
    let active_preferred = preferred_type.unwrap_or(platform_default);

    // Build provider list from static/compiled-in knowledge. Does NOT call into
    // ORT (no list_devices, no ort::environment::current) so this is instant.
    // ORT device enumeration is only needed for the Settings detail view
    // (list_ort_devices command), not for determining which EP is active.

    #[cfg(feature = "cuda")]
    let cuda_state = probe_cuda_available();
    #[cfg(feature = "cuda")]
    let cuda_available = cuda_state.is_available();

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

    // CUDA - only when cuda feature is enabled at compile time
    #[cfg(feature = "cuda")]
    providers.push(ExecutionProviderStatus {
        provider: ExecutionProviderType::Cuda,
        available: cuda_available,
        reason: if cuda_available {
            // In-process NVML query - no nvidia-smi spawn, no 0xC0000142 popup.
            crate::gpu_probe::primary_gpu_name().unwrap_or_else(|| "NVIDIA GPU".to_string())
        } else {
            cuda_unavailable_hint()
        },
        requires_download: false,
        hard_incompatible: cuda_state.is_hard_incompat(),
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
            ExecutionProviderType::Cuda => "Using CUDA".to_string(),
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
/// Fallback: preferred -> platform default (DirectML/CoreML) -> CUDA -> CPU
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

    if preferred != ExecutionProviderType::Cuda && platform_default != ExecutionProviderType::Cuda {
        if providers
            .iter()
            .any(|p| p.provider == ExecutionProviderType::Cuda && p.available)
        {
            return ExecutionProviderType::Cuda;
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
        assert_eq!(ExecutionProviderType::Cuda.to_string(), "CUDA");
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
        assert!(ExecutionProviderType::Cuda.is_gpu());
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
}
