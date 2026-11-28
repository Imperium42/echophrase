// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use std::path::PathBuf;

// ============================================================================
// Shared Path Utilities for Model Detection
// ============================================================================

/// Returns the HuggingFace hub cache directory.
/// Default location: ~/.cache/huggingface/hub/
pub fn get_hf_cache_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .ok_or_else(|| "Could not find home directory".to_string())
        .map(|home| home.join(".cache").join("huggingface").join("hub"))
}

/// Converts repo_id to HuggingFace cache directory name.
/// Example: "distil-whisper/distil-large-v3" → "models--distil-whisper--distil-large-v3"
pub fn repo_to_cache_name(repo_id: &str) -> String {
    format!("models--{}", repo_id.replace('/', "--"))
}

/// Returns the path to a model's snapshot directory within the HF cache.
/// Checks for 'main' symlink first, then falls back to finding the latest snapshot.
pub fn get_model_snapshot_dir(cache_dir: &PathBuf, repo_id: &str) -> Option<PathBuf> {
    let cache_name = repo_to_cache_name(repo_id);
    let snapshots_dir = cache_dir.join(&cache_name).join("snapshots");

    if !snapshots_dir.exists() {
        return None;
    }

    // Prefer 'main' symlink if it exists (standard HF cache structure)
    let main_dir = snapshots_dir.join("main");
    if main_dir.exists() {
        return Some(main_dir);
    }

    // Fall back to finding any snapshot directory (hash-named)
    if let Ok(entries) = std::fs::read_dir(&snapshots_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_to_cache_name() {
        assert_eq!(
            repo_to_cache_name("distil-whisper/distil-large-v3"),
            "models--distil-whisper--distil-large-v3"
        );
        assert_eq!(
            repo_to_cache_name("openai/whisper-large-v3-turbo"),
            "models--openai--whisper-large-v3-turbo"
        );
        assert_eq!(
            repo_to_cache_name("istupakov/parakeet-tdt-0.6b-v3-onnx"),
            "models--istupakov--parakeet-tdt-0.6b-v3-onnx"
        );
    }

    #[test]
    fn test_get_hf_cache_dir() {
        let result = get_hf_cache_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.ends_with("hub"));
    }
}
