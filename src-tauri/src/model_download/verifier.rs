// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use crate::commands::transcription::log_debug;

/// Recursively sum sizes of all `.partial` files under `dir`.
/// Multi-file model downloads can have multiple partials in nested
/// subdirectories (e.g. parakeet-lite stores files under
/// `snapshots/main/realtime_eou_120m-v1-onnx/`), so a flat scan misses them.
pub fn sum_partial_files(dir: &std::path::Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += sum_partial_files(&path);
            } else if path.extension().map(|e| e == "partial").unwrap_or(false) {
                if let Ok(metadata) = std::fs::metadata(&path) {
                    log_debug(format!("📦 Partial: {:?} ({} bytes)", path, metadata.len()));
                    total += metadata.len();
                }
            }
        }
    }
    total
}
