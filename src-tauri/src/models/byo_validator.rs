// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Validation pipeline for BYO (bring-your-own) custom models.
//!
//! v1 accepts ONNX only. The authoritative check is loading the file in a
//! throwaway ONNX Runtime session - a content check, not an extension check,
//! so garbage renamed to `.onnx` is rejected.
//!
//! TODO(safetensors): when the Candle backend returns (ONNX_ONLY_MODE lifted),
//! re-add structural safetensors validation here (8-byte LE header length +
//! JSON header parse, never loading weights) and accept `.safetensors` imports.

use serde::Serialize;
use std::path::Path;

/// Maximum accepted custom model file size (8 GB). The largest registry model
/// today is ~2.5 GB; this cap exists to fail fast on absurd files, not to be
/// a tight bound.
pub const MAX_CUSTOM_MODEL_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// Maximum length of a sanitized model directory name.
const MAX_SANITIZED_NAME_LEN: usize = 64;

/// Typed error code for BYO import failures. Mirrored in TypeScript
/// (`src/lib/types/models.ts`) and mapped to i18n keys - keep in sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[allow(dead_code)] // Variants mirror the TS union 1:1 even when currently unconstructed.
pub enum ByoErrorCode {
    /// File is not a supported model format (v1: `.onnx` only).
    UnsupportedFormat,
    /// File exceeds [`MAX_CUSTOM_MODEL_BYTES`].
    FileTooLarge,
    /// Extension is right but ONNX Runtime rejected the content.
    CorruptOrInvalid,
    /// Source path is a symlink - rejected to avoid path games.
    SymlinkRejected,
    /// HuggingFace repo has more than one `.onnx` file (ambiguous in v1).
    HfRepoAmbiguous,
    /// HuggingFace repo missing, private, or has no `.onnx` file.
    HfRepoNotFound,
    /// Network failure talking to HuggingFace.
    NetworkError,
    /// Input contained path traversal or other forbidden characters.
    PathTraversalRejected,
    /// No custom model is currently imported.
    NotImported,
    /// This exact source (local file path or HF repo) is already imported.
    DuplicateSource,
    /// Another model download (registry or BYO) is already running - see
    /// `DOWNLOAD_SESSION_LOCK` in `commands::transcription`.
    DownloadInProgress,
    /// Unexpected internal failure (details logged, not surfaced).
    Internal,
}

/// A typed import error: `code` drives frontend i18n copy, `message` is a
/// developer-readable detail (safe to show, never contains raw paths from
/// untrusted input).
#[derive(Debug, Clone, Serialize)]
pub struct ByoError {
    pub code: ByoErrorCode,
    pub message: String,
}

impl ByoError {
    /// Build an error with the given code and message.
    pub fn new(code: ByoErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Serialize to the JSON string returned through the Tauri command
    /// boundary (commands return `Result<_, String>`).
    pub fn to_command_error(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| {
            format!("{{\"code\":\"Internal\",\"message\":\"{:?}\"}}", self.code)
        })
    }
}

/// Sanitize a user-provided display name into a filesystem-safe directory
/// name: only `[A-Za-z0-9_-]` survive, everything else becomes `-`, runs are
/// collapsed, length capped, and an empty result falls back to
/// `custom-model`. Never produces `..`, separators, null bytes, or a leading
/// dot by construction.
pub fn sanitize_model_dir_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len().min(MAX_SANITIZED_NAME_LEN));
    let mut last_dash = false;
    for c in name.chars() {
        if out.len() >= MAX_SANITIZED_NAME_LEN {
            break;
        }
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "custom-model".to_string()
    } else {
        trimmed
    }
}

/// Validate a HuggingFace repo id of the form `org/repo`. Accepts the two
/// path segments HF allows (`[A-Za-z0-9._-]`, no `..` segments) and nothing
/// else - this string is later interpolated into API URLs.
pub fn validate_repo_id(repo_id: &str) -> Result<(), ByoError> {
    let parts: Vec<&str> = repo_id.split('/').collect();
    let valid_segment = |s: &str| {
        !s.is_empty()
            && s != ".."
            && s != "."
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    if parts.len() == 2 && parts.iter().all(|s| valid_segment(s)) {
        Ok(())
    } else {
        Err(ByoError::new(
            ByoErrorCode::PathTraversalRejected,
            "Repo id must look like org/repo",
        ))
    }
}

/// Validate a file path *within* a HuggingFace repo tree (eg
/// `assets/model.onnx`, extracted from a `/blob/main/...` or
/// `/resolve/main/...` URL). Unlike [`validate_repo_id`] this permits nested
/// `/`-separated segments (real HF repos nest files in subdirectories), but
/// applies the same per-segment allowlist and rejects `..`/`.` segments and
/// anything not ending in `.onnx`. Not a filesystem traversal risk on its own
/// (the local destination filename is always the fixed `MODEL_FILE_NAME`
/// constant, never derived from this path) - this exists to reject garbage
/// input early with a clear error instead of it silently 404ing later.
pub fn validate_repo_file_path(path: &str) -> Result<(), ByoError> {
    let valid_segment = |s: &str| {
        !s.is_empty()
            && s != ".."
            && s != "."
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    let segments: Vec<&str> = path.split('/').collect();
    if !path.to_ascii_lowercase().ends_with(".onnx") || !segments.iter().all(|s| valid_segment(s)) {
        return Err(ByoError::new(
            ByoErrorCode::PathTraversalRejected,
            "That does not look like a valid path to an .onnx file in the repo",
        ));
    }
    Ok(())
}

/// Pre-flight checks on a local source file: rejects symlinks (checked via
/// `symlink_metadata`, never following the link), enforces the size cap, and
/// requires a `.onnx` extension. Content validation happens separately in
/// [`validate_onnx_file`].
pub fn preflight_local_file(path: &Path) -> Result<u64, ByoError> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| {
        ByoError::new(
            ByoErrorCode::CorruptOrInvalid,
            format!("Cannot read file: {}", e),
        )
    })?;
    if meta.file_type().is_symlink() {
        return Err(ByoError::new(
            ByoErrorCode::SymlinkRejected,
            "Symlinks are not accepted - pick the real file",
        ));
    }
    if !meta.is_file() {
        return Err(ByoError::new(
            ByoErrorCode::CorruptOrInvalid,
            "Not a regular file",
        ));
    }
    if meta.len() > MAX_CUSTOM_MODEL_BYTES {
        return Err(ByoError::new(
            ByoErrorCode::FileTooLarge,
            format!(
                "File is {} bytes; the limit is {} bytes",
                meta.len(),
                MAX_CUSTOM_MODEL_BYTES
            ),
        ));
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("onnx") => Ok(meta.len()),
        // TODO(safetensors): accept + park when the Candle backend returns.
        Some("safetensors") => Err(ByoError::new(
            ByoErrorCode::UnsupportedFormat,
            "safetensors models are not supported yet - ONNX only for now",
        )),
        _ => Err(ByoError::new(
            ByoErrorCode::UnsupportedFormat,
            "Only .onnx model files are supported",
        )),
    }
}

/// Authoritative ONNX validation: build a throwaway CPU-only ONNX Runtime
/// session from the file. Slow for huge models, but imports are rare and
/// user-initiated (the UI shows a validating spinner). Callers MUST run this
/// on a blocking thread (`spawn_blocking`). Panics from ORT are caught, same
/// as the production session path in `onnx_backend.rs`.
pub fn validate_onnx_file(path: &Path) -> Result<(), ByoError> {
    if !crate::is_onnx_runtime_available() {
        return Err(ByoError::new(
            ByoErrorCode::Internal,
            "ONNX Runtime is not available on this system",
        ));
    }
    let path = path.to_path_buf();
    let result = std::panic::catch_unwind(move || -> Result<(), String> {
        use ort::session::Session;
        let mut builder = Session::builder().map_err(|e| e.to_string())?;
        // No execution providers registered: implicit CPU. Validation only
        // needs "does ORT accept this graph", not fast inference.
        builder.commit_from_file(&path).map_err(|e| e.to_string())?;
        Ok(())
    });
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            log::warn!("[byo] ONNX validation rejected file: {}", e);
            Err(ByoError::new(
                ByoErrorCode::CorruptOrInvalid,
                "ONNX Runtime could not load this file - it does not appear to be a valid ONNX model",
            ))
        }
        Err(panic) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "unknown panic".to_string());
            log::error!("[byo] ONNX Runtime panicked validating file: {}", msg);
            Err(ByoError::new(
                ByoErrorCode::CorruptOrInvalid,
                "ONNX Runtime crashed loading this file",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// ORT session tests need the dynamic runtime actually loadable. In a bare
    /// `cargo test` env ORT_DYLIB_PATH is unset (it comes from env.yaml via
    /// the dev scripts), and ort's dynamic loader BLOCKS without it - so skip
    /// unless the dylib is really there. Run with
    /// `ORT_DYLIB_PATH=.ort/<platform>/lib/libonnxruntime.so cargo test` to
    /// exercise them.
    fn ort_ready() -> bool {
        crate::is_onnx_runtime_available()
            && std::env::var("ORT_DYLIB_PATH")
                .map(|p| std::path::Path::new(&p).exists())
                .unwrap_or(false)
    }

    #[test]
    fn sanitizer_strips_traversal_and_separators() {
        assert_eq!(sanitize_model_dir_name("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitize_model_dir_name("a/b\\c"), "a-b-c");
        assert_eq!(sanitize_model_dir_name("nul\0byte"), "nul-byte");
        assert_eq!(sanitize_model_dir_name(".hidden"), "hidden");
    }

    #[test]
    fn sanitizer_falls_back_when_empty() {
        assert_eq!(sanitize_model_dir_name(""), "custom-model");
        assert_eq!(sanitize_model_dir_name("///..."), "custom-model");
    }

    #[test]
    fn sanitizer_caps_length() {
        let long = "x".repeat(500);
        assert!(sanitize_model_dir_name(&long).len() <= MAX_SANITIZED_NAME_LEN);
    }

    #[test]
    fn repo_id_validation() {
        assert!(validate_repo_id("openai/whisper-tiny.en").is_ok());
        assert!(validate_repo_id("org/repo").is_ok());
        assert!(validate_repo_id("org/../repo").is_err());
        assert!(validate_repo_id("org/repo/extra").is_err());
        assert!(validate_repo_id("noslash").is_err());
        assert!(validate_repo_id("org/re po").is_err());
        assert!(validate_repo_id("/repo").is_err());
    }

    #[test]
    fn preflight_rejects_wrong_extension() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.gguf");
        std::fs::File::create(&path).unwrap();
        let err = preflight_local_file(&path).unwrap_err();
        assert_eq!(err.code, ByoErrorCode::UnsupportedFormat);
    }

    #[test]
    fn preflight_rejects_safetensors_for_now() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.safetensors");
        std::fs::File::create(&path).unwrap();
        let err = preflight_local_file(&path).unwrap_err();
        assert_eq!(err.code, ByoErrorCode::UnsupportedFormat);
    }

    #[cfg(unix)]
    #[test]
    fn preflight_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.onnx");
        std::fs::File::create(&target).unwrap();
        let link = dir.path().join("link.onnx");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let err = preflight_local_file(&link).unwrap_err();
        assert_eq!(err.code, ByoErrorCode::SymlinkRejected);
    }

    #[test]
    fn garbage_with_onnx_extension_rejected_by_content_check() {
        if !ort_ready() {
            return; // No ORT dylib in this test environment - content check is moot.
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("garbage.onnx");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"this is definitely not protobuf").unwrap();
        drop(f);
        let err = validate_onnx_file(&path).unwrap_err();
        assert_eq!(err.code, ByoErrorCode::CorruptOrInvalid);
    }

    #[test]
    fn real_onnx_file_accepted() {
        if !ort_ready() {
            return; // No ORT dylib in this test environment.
        }
        // Bundled Silero VAD model doubles as a small known-good ONNX fixture.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("silero_vad.onnx");
        assert!(validate_onnx_file(&path).is_ok());
    }

    #[test]
    fn error_serializes_with_code() {
        let e = ByoError::new(ByoErrorCode::FileTooLarge, "too big");
        let s = e.to_command_error();
        assert!(s.contains("FileTooLarge"));
        assert!(s.contains("too big"));
    }
}
