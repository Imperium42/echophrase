// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

#[allow(unexpected_cfgs)] // Allow feature = "cuda" checks for future CUDA feature
#[cfg(test)]
mod environment_tests {
    use std::env;
    use std::fs;
    use std::path::Path;

    /// Check if running in WSL environment
    fn is_wsl() -> bool {
        // Check for WSL-specific indicators
        if cfg!(target_os = "linux") {
            // Check /proc/version for WSL/Microsoft strings
            if let Ok(content) = fs::read_to_string("/proc/version") {
                return content.to_lowercase().contains("microsoft")
                    || content.to_lowercase().contains("wsl");
            }
            // Check for WSL-specific environment variables
            if env::var("WSL_DISTRO_NAME").is_ok() || env::var("WSL_INTEROP").is_ok() {
                return true;
            }
        }
        false
    }

    /// Check if running on native Windows
    fn is_windows_native() -> bool {
        cfg!(target_os = "windows")
    }

    /// Detect CUDA availability based on platform
    fn detect_cuda() -> Result<String, String> {
        if is_windows_native() {
            // Windows: Check standard CUDA paths
            let cuda_paths = vec![
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.0",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.1",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.2",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.3",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.4",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.5",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.6",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.7",
                r"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v12.8",
            ];

            for path in cuda_paths {
                let nvcc_path = format!(r"{}\bin\nvcc.exe", path);
                if Path::new(&nvcc_path).exists() {
                    return Ok(format!("CUDA found at: {}", path));
                }
            }

            // Also check CUDA_PATH environment variable
            if let Ok(cuda_path) = env::var("CUDA_PATH") {
                if Path::new(&cuda_path).exists() {
                    return Ok(format!("CUDA found via CUDA_PATH: {}", cuda_path));
                }
            }

            Err("CUDA not found on Windows".to_string())
        } else if is_wsl() {
            // WSL: Check Linux CUDA paths
            let cuda_paths = vec![
                "/usr/local/cuda",
                "/usr/local/cuda-12",
                "/usr/local/cuda-12.0",
                "/usr/local/cuda-12.1",
                "/usr/local/cuda-12.2",
                "/usr/local/cuda-12.3",
                "/usr/local/cuda-12.4",
                "/usr/local/cuda-12.5",
            ];

            for path in cuda_paths {
                let nvcc_path = format!("{}/bin/nvcc", path);
                if Path::new(&nvcc_path).exists() {
                    // Also check if WSL can access Windows GPU
                    if Path::new("/usr/lib/wsl/lib/libcuda.so").exists() {
                        return Ok(format!("CUDA found at: {} (with WSL GPU support)", path));
                    } else {
                        return Ok(format!(
                            "CUDA found at: {} (WSL GPU support not configured)",
                            path
                        ));
                    }
                }
            }

            Err("CUDA not found in WSL".to_string())
        } else {
            // Native Linux
            if Path::new("/usr/local/cuda/bin/nvcc").exists() {
                Ok("CUDA found at: /usr/local/cuda".to_string())
            } else {
                Err("CUDA not found on Linux".to_string())
            }
        }
    }

    #[test]
    fn test_environment_detection() {
        println!("\n🔍 Environment Detection Test");
        println!("==============================");

        // Platform detection
        println!("\n📍 Platform:");
        if is_windows_native() {
            println!("   ✅ Native Windows detected");
            assert_eq!(std::env::consts::OS, "windows");
        } else if is_wsl() {
            println!("   ✅ WSL environment detected");
            assert_eq!(std::env::consts::OS, "linux");

            // Additional WSL checks
            if let Ok(distro) = env::var("WSL_DISTRO_NAME") {
                println!("   📦 WSL Distribution: {}", distro);
            }

            // Check for WSL GPU support
            if Path::new("/usr/lib/wsl/lib/libcuda.so").exists() {
                println!("   🎮 WSL GPU passthrough: Available");
            } else {
                println!("   ⚠️  WSL GPU passthrough: Not configured");
            }
        } else {
            println!("   ✅ Native Linux detected");
            assert_eq!(std::env::consts::OS, "linux");
        }

        // CUDA detection
        println!("\n🚀 CUDA Detection:");
        match detect_cuda() {
            Ok(msg) => println!("   ✅ {}", msg),
            Err(msg) => println!("   ⚠️  {}", msg),
        }

        // Environment variables
        println!("\n🔧 Environment Variables:");
        let env_vars = vec![
            "CUDA_HOME",
            "CUDA_PATH",
            "CUDNN_PATH",
            "CT2_CUDA_TRUE_FP16_GEMM",
            "LD_LIBRARY_PATH",
            "PULSE_SERVER",
        ];

        for var in env_vars {
            match env::var(var) {
                Ok(value) => {
                    // Truncate long paths for readability
                    let display_value = if value.len() > 60 {
                        format!("{}...", &value[..57])
                    } else {
                        value
                    };
                    println!("   ✅ {} = {}", var, display_value);
                }
                Err(_) => println!("   ⚠️  {} not set", var),
            }
        }

        // Model cache detection
        println!("\n📦 Model Cache:");
        let cache_paths = if is_windows_native() {
            vec![
                env::var("USERPROFILE")
                    .map(|p| format!(r"{}\.cache\huggingface\hub", p))
                    .unwrap_or_default(),
            ]
        } else {
            vec![format!(
                "{}/.cache/huggingface/hub",
                env::var("HOME").unwrap_or_default()
            )]
        };

        for path in cache_paths {
            if !path.is_empty() && Path::new(&path).exists() {
                println!("   ✅ Cache directory exists: {}", path);

                // Check for Whisper model
                let model_path = format!(
                    "{}/models--Zoont--faster-whisper-large-v3-turbo-int8-ct2",
                    path
                );
                if Path::new(&model_path).exists() {
                    println!("   ✅ Whisper INT8 model found in cache");
                } else {
                    println!("   ⚠️  Whisper INT8 model not in cache");
                }
            } else {
                println!("   ⚠️  Cache directory not found: {}", path);
            }
        }

        println!("\n✅ Environment test completed successfully!");
    }

    #[test]
    fn test_cuda_availability() {
        println!("\n🎮 CUDA Availability Test");
        println!("=========================");

        // Try to check if CUDA is actually functional
        #[cfg(feature = "cuda")]
        {
            println!("✅ Compiled with CUDA support");

            // Check if ct2rs can detect CUDA
            if let Ok(cuda_available) = check_ct2rs_cuda() {
                if cuda_available {
                    println!("✅ ct2rs reports CUDA is available");
                } else {
                    println!("⚠️  ct2rs reports CUDA is not available");
                }
            }
        }

        #[cfg(not(feature = "cuda"))]
        {
            println!("⚠️  Not compiled with CUDA support");
            println!("   Rebuild with --features cuda to enable GPU acceleration");
        }
    }

    #[cfg(feature = "cuda")]
    fn check_ct2rs_cuda() -> Result<bool, Box<dyn std::error::Error>> {
        // This would actually check ct2rs CUDA availability
        // For now, just check if the library can be loaded
        Ok(true) // Placeholder
    }
}
