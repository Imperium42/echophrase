// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

#[cfg(test)]
mod environment_tests {
    use std::env;
    use std::fs;

    /// Check if running in WSL environment
    fn is_wsl() -> bool {
        if cfg!(target_os = "linux") {
            // Check /proc/version for WSL/Microsoft strings
            if let Ok(content) = fs::read_to_string("/proc/version") {
                let content = content.to_lowercase();
                return content.contains("microsoft") || content.contains("wsl");
            }
            // Check for WSL-specific environment variables
            if env::var("WSL_DISTRO_NAME").is_ok() || env::var("WSL_INTEROP").is_ok() {
                return true;
            }
        }
        false
    }

    #[test]
    fn test_environment_detection() {
        if cfg!(target_os = "windows") {
            println!("Native Windows detected");
            assert_eq!(std::env::consts::OS, "windows");
        } else if is_wsl() {
            println!("WSL environment detected");
            assert_eq!(std::env::consts::OS, "linux");
            if let Ok(distro) = env::var("WSL_DISTRO_NAME") {
                println!("WSL distribution: {distro}");
            }
        } else if cfg!(target_os = "macos") {
            println!("macOS detected");
            assert_eq!(std::env::consts::OS, "macos");
        } else {
            println!("Native Linux detected");
            assert_eq!(std::env::consts::OS, "linux");
        }

        for var in ["ORT_DYLIB_PATH", "LD_LIBRARY_PATH", "PULSE_SERVER"] {
            match env::var(var) {
                Ok(value) => println!("{var} = {value}"),
                Err(_) => println!("{var} not set"),
            }
        }
    }
}
