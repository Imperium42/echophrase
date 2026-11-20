// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Tauri commands for the BYO custom model feature.
//!
//! Import a custom ONNX transcription model from a local file or a
//! HuggingFace repo, validate it (see `models::byo_validator`), and store it
//! under `~/.cache/echophrase/custom-models/<sanitized-name>/` with fixed
//! file names so the runtime never resolves user-controlled paths:
//!
//! ```text
//! custom-models/<name>/
//!   model.onnx        # primary, renamed on copy
//!   model.onnx.data   # optional external-data companion
//!   tokenizer.json    # optional
//!   config.json       # optional
//!   meta.json         # import metadata (source, size, timestamps)
//! ```
//!
//! No SHA pinning here by design: BYO models are user-chosen, so there is no
//! trusted manifest to pin against (same policy as documented for registry
//! downloads in `model_download/manifest.rs` - the trust decision is the
//! user's, our job is structural validation + honest errors).

use crate::models::byo_validator::{
    ByoError, ByoErrorCode, MAX_CUSTOM_MODEL_BYTES, preflight_local_file, sanitize_model_dir_name,
    validate_onnx_file, validate_repo_file_path, validate_repo_id,
};
use crate::settings::AppSettings;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

/// Fixed on-disk name of the imported model file.
const MODEL_FILE_NAME: &str = "model.onnx";
/// Fixed on-disk name of the optional external-data companion. ORT resolves
/// external data relative to the model file, so keeping our own fixed name
/// confines resolution to our controlled directory.
const MODEL_DATA_FILE_NAME: &str = "model.onnx.data";
/// Optional companion files fetched from HF repos (fixed allowlist - HF API
/// filenames are never used to build local paths).
const COMPANION_FILES: [&str; 2] = ["tokenizer.json", "config.json"];

/// Import metadata persisted as `meta.json` and surfaced in Settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelInfo {
    /// User-facing display name.
    pub name: String,
    /// Model format - always "onnx" in v1.
    pub format: String,
    /// Size of the primary model file in bytes.
    pub size_bytes: u64,
    /// "file" or a HuggingFace repo id like "org/repo".
    pub source: String,
    /// Original filename at import time (display only, never used in paths).
    pub original_filename: String,
    /// RFC3339 import timestamp.
    pub imported_at: String,
    /// Whether the current backend can load this model (always true in v1 -
    /// only loadable formats are accepted).
    pub is_loadable: bool,
    /// Canonicalized source file path for local imports (`None` for HF imports
    /// and for models imported before this field existed - those simply do not
    /// participate in duplicate detection). Never used to resolve paths.
    #[serde(default)]
    pub source_path: Option<String>,
}

/// A discovered imported model: its metadata plus the managed directory it
/// lives in. The directory path is the stable identifier used by the
/// select/remove commands.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelEntry {
    /// Parsed `meta.json` contents.
    pub info: CustomModelInfo,
    /// Absolute path of the model directory under the custom models root.
    pub path: String,
}

/// Root directory for all imported custom models. Distinct from the legacy
/// GGUF-era `~/.cache/echophrase/models/custom` path.
fn custom_models_root() -> Result<PathBuf, ByoError> {
    dirs::cache_dir()
        .map(|d| d.join("echophrase").join("custom-models"))
        .ok_or_else(|| ByoError::new(ByoErrorCode::Internal, "No cache directory available"))
}

/// Write `meta.json` and point settings at the imported model. Also selects
/// nothing automatically - the user picks "Custom Model" in the model list.
fn finalize_import(
    model_dir: &Path,
    info: &CustomModelInfo,
    source_url: Option<String>,
) -> Result<(), ByoError> {
    let meta_json = serde_json::to_string_pretty(info)
        .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
    std::fs::write(model_dir.join("meta.json"), meta_json)
        .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;

    let dir_string = model_dir.to_string_lossy().to_string();
    let name = info.name.clone();
    AppSettings::update(|s| {
        s.custom_model_path = Some(dir_string.clone());
        s.custom_model_format = Some(info.format.clone());
        s.custom_model_name = Some(name.clone());
        s.custom_model_url = source_url.clone();
    })
    .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
    Ok(())
}

/// Verify that `dir` is a real (non-symlink) directory sitting DIRECTLY under
/// the custom-models root, resolving symlinks via canonicalization so a
/// planted link inside the root cannot point deletion/selection elsewhere.
/// Returns the canonicalized path. The path comes from the webview, which is
/// only semi-trusted - textual `starts_with` alone is not enough.
fn verify_managed_model_dir(dir: &Path) -> Result<PathBuf, ByoError> {
    let traversal = |msg: &str| ByoError::new(ByoErrorCode::PathTraversalRejected, msg);
    let meta =
        std::fs::symlink_metadata(dir).map_err(|_| traversal("Model directory does not exist"))?;
    if meta.file_type().is_symlink() {
        return Err(traversal("Model directory must not be a symlink"));
    }
    let root = std::fs::canonicalize(custom_models_root()?)
        .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
    let canonical = std::fs::canonicalize(dir)
        .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
    // Managed dirs are always exactly root/<sanitized-name>: requiring the
    // canonical parent to BE the canonical root is stricter than starts_with.
    if canonical.parent() != Some(root.as_path()) {
        return Err(traversal(
            "Model directory is outside the custom models root",
        ));
    }
    Ok(canonical)
}

/// Allocate a FREE directory for a new import: `root/<sanitized-name>`, or
/// `<sanitized-name>-2`, `-3`, ... when taken. Two different models may share
/// a display name (ONNX exports are routinely just "model.onnx"), and a new
/// import must never clobber an existing entry's files.
fn unique_model_dir(display_name: &str) -> Result<PathBuf, ByoError> {
    let root = custom_models_root()?;
    let base = sanitize_model_dir_name(display_name);
    let mut candidate = root.join(&base);
    let mut n = 1u32;
    while candidate.exists() {
        n += 1;
        candidate = root.join(format!("{}-{}", base, n));
    }
    Ok(candidate)
}

/// Directory for a resumable HF download: `root/<sanitized-name>`, reused
/// as-is if it already exists but has no `meta.json` (an abandoned/cancelled
/// download - its `.partial` file should be resumed into, not orphaned by
/// `unique_model_dir` handing out a fresh `-2` sibling every retry). Only
/// bumps to `-2`/`-3`... when the existing directory is a REAL completed,
/// distinct import (has `meta.json`) - never touches or clobbers that.
fn resumable_model_dir(display_name: &str) -> Result<PathBuf, ByoError> {
    let root = custom_models_root()?;
    let base = sanitize_model_dir_name(display_name);
    let mut candidate = root.join(&base);
    let mut n = 1u32;
    while candidate.exists() && candidate.join("meta.json").exists() {
        n += 1;
        candidate = root.join(format!("{}-{}", base, n));
    }
    Ok(candidate)
}

/// Scan the managed root for directories with a readable `meta.json`. Shared
/// by `list_custom_models` and import-time duplicate detection.
fn scan_custom_models() -> Vec<CustomModelEntry> {
    let mut models: Vec<CustomModelEntry> = Vec::new();
    let Ok(root) = custom_models_root() else {
        return models;
    };
    let Ok(dir) = std::fs::read_dir(&root) else {
        return models;
    };
    for entry in dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(path.join("meta.json")) else {
            continue;
        };
        match serde_json::from_str::<CustomModelInfo>(&contents) {
            Ok(info) => models.push(CustomModelEntry {
                info,
                path: path.to_string_lossy().to_string(),
            }),
            Err(e) => log::warn!("[byo] Skipping unreadable meta.json in {:?}: {}", path, e),
        }
    }
    models
}

/// Find an already-imported model whose recorded canonical source path matches
/// this local import. Path-based only, by design: the same bytes imported from
/// a different location are allowed.
fn find_duplicate_local<'a>(
    models: &'a [CustomModelEntry],
    canonical_source: &str,
) -> Option<&'a CustomModelEntry> {
    models
        .iter()
        .find(|m| m.info.source_path.as_deref() == Some(canonical_source))
}

/// Find an already-imported model that came from the same HuggingFace repo.
fn find_duplicate_hf<'a>(
    models: &'a [CustomModelEntry],
    repo_id: &str,
) -> Option<&'a CustomModelEntry> {
    models.iter().find(|m| m.info.source == repo_id)
}

/// Build the typed duplicate-import error, naming the existing model.
fn duplicate_error(existing: &CustomModelEntry) -> ByoError {
    ByoError::new(
        ByoErrorCode::DuplicateSource,
        format!("Already imported as '{}'", existing.info.name),
    )
}

/// Remove a previously imported model directory, verifying it actually lives
/// under our custom-models root before deleting anything.
fn remove_model_dir(dir: &Path) -> Result<(), ByoError> {
    if !dir.exists() {
        return Ok(());
    }
    let canonical = verify_managed_model_dir(dir)?;
    std::fs::remove_dir_all(canonical)
        .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
    Ok(())
}

/// True when the just-observed failure was a user cancel (Pause), not a real
/// error. Mirrors the check `import_custom_model_from_url` makes at the
/// command boundary - duplicated here because by the time that boundary
/// runs, the directory would already be deleted otherwise.
fn was_cancelled() -> bool {
    crate::commands::transcription::DOWNLOAD_CANCELLED.load(std::sync::atomic::Ordering::SeqCst)
}

/// Clean up a failed import directory - but only for a REAL failure. A user
/// cancel must leave the directory (and any `.partial` file inside it) so a
/// later import of the same repo/file resumes via HTTP Range instead of
/// restarting from zero. Mirrors `download_registry_model`'s `Ok(())`-on-cancel
/// branch (`model_download/downloader.rs`), which never touches its directory.
fn cleanup_after_failed_import(model_dir: &Path) {
    if was_cancelled() {
        crate::commands::transcription::log_debug(format!(
            "[byo] Download cancelled - preserving {:?} for resume",
            model_dir
        ));
        return;
    }
    let _ = remove_model_dir(model_dir);
}

/// Copy + validate a local model file (and optional `.onnx.data` companion)
/// into the managed directory. Blocking work runs on a blocking thread.
async fn import_local(
    source_path: String,
    companion_data_path: Option<String>,
    display_name: String,
) -> Result<CustomModelInfo, ByoError> {
    tauri::async_runtime::spawn_blocking(move || -> Result<CustomModelInfo, ByoError> {
        let source = PathBuf::from(&source_path);
        let size_bytes = preflight_local_file(&source)?;

        // Duplicate check by canonical source PATH (not content): re-importing
        // the same file is rejected; the same bytes from elsewhere are fine.
        // `dunce::canonicalize` (not `std::fs::canonicalize`) - the std version
        // prefixes Windows paths with `\\?\` (and `\\?\UNC\` for network/WSL
        // paths), which is technically correct but leaks an ugly, fragile
        // path into meta.json and dedup comparisons. dunce strips it when the
        // plain form is unambiguous, same path on both Windows and Unix.
        let canonical_source = dunce::canonicalize(&source)
            .unwrap_or_else(|_| source.clone())
            .to_string_lossy()
            .to_string();
        if let Some(existing) = find_duplicate_local(&scan_custom_models(), &canonical_source) {
            return Err(duplicate_error(existing));
        }

        // Companion pre-flight: must be a regular non-symlink file too.
        let companion = match &companion_data_path {
            Some(p) => {
                let path = PathBuf::from(p);
                let meta = std::fs::symlink_metadata(&path).map_err(|e| {
                    ByoError::new(
                        ByoErrorCode::CorruptOrInvalid,
                        format!("Cannot read companion file: {}", e),
                    )
                })?;
                if meta.file_type().is_symlink() || !meta.is_file() {
                    return Err(ByoError::new(
                        ByoErrorCode::SymlinkRejected,
                        "Companion file must be a regular file",
                    ));
                }
                if meta.len() > MAX_CUSTOM_MODEL_BYTES {
                    return Err(ByoError::new(
                        ByoErrorCode::FileTooLarge,
                        "Companion file exceeds the size limit",
                    ));
                }
                Some(path)
            }
            None => None,
        };

        let model_dir = unique_model_dir(&display_name)?;
        std::fs::create_dir_all(&model_dir)
            .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;

        // std::fs::copy streams through the kernel - never loads the file in RAM.
        let dest = model_dir.join(MODEL_FILE_NAME);
        std::fs::copy(&source, &dest)
            .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
        if let Some(companion_src) = companion {
            std::fs::copy(&companion_src, model_dir.join(MODEL_DATA_FILE_NAME))
                .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
        }

        // Validate the COPY in its final layout, so external-data resolution
        // is exercised exactly as it will be at transcription time.
        if let Err(e) = validate_onnx_file(&dest) {
            let _ = remove_model_dir(&model_dir);
            return Err(e);
        }

        let original_filename = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let info = CustomModelInfo {
            name: display_name,
            format: "onnx".to_string(),
            size_bytes,
            source: "file".to_string(),
            original_filename,
            imported_at: chrono::Utc::now().to_rfc3339(),
            is_loadable: true,
            source_path: Some(canonical_source),
        };
        finalize_import(&model_dir, &info, None)?;
        Ok(info)
    })
    .await
    .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?
}

/// One entry in the HuggingFace repo tree API response.
#[derive(Debug, Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    path: String,
    #[serde(default)]
    size: u64,
}

/// Parse user input (`org/repo`, a bare repo URL, or a direct
/// `/blob/<rev>/<path>` or `/resolve/<rev>/<path>` file link) into a
/// validated repo id plus an optional explicit file path within it. `None`
/// means the caller must resolve candidates (see `list_hf_onnx_files`);
/// `Some` means the user already pointed at one exact file.
fn parse_hf_input(input: &str) -> Result<(String, Option<String>), ByoError> {
    let trimmed = input.trim().trim_end_matches('/');
    let rest = trimmed
        .strip_prefix("https://huggingface.co/")
        .or_else(|| trimmed.strip_prefix("http://huggingface.co/"))
        .or_else(|| trimmed.strip_prefix("huggingface.co/"));

    let Some(rest) = rest else {
        // No recognized host prefix - treat the whole input as `org/repo`.
        let repo = trimmed.to_string();
        validate_repo_id(&repo)?;
        return Ok((repo, None));
    };

    let segments: Vec<&str> = rest.split('/').collect();
    let repo = segments
        .iter()
        .take(2)
        .copied()
        .collect::<Vec<_>>()
        .join("/");
    validate_repo_id(&repo)?;

    // Look for a `/blob/<rev>/...` or `/resolve/<rev>/...` marker after the
    // repo id - everything past it is the file path within the repo.
    let file_path = segments
        .get(2)
        .filter(|marker| **marker == "blob" || **marker == "resolve")
        .and_then(|_| segments.get(4..))
        .map(|rest| rest.join("/"))
        .filter(|p| !p.is_empty());

    if let Some(path) = &file_path {
        validate_repo_file_path(path)?;
    }

    Ok((repo, file_path))
}

/// Fetch a HuggingFace repo's full file tree (recursive).
async fn fetch_hf_tree(
    client: &reqwest::Client,
    repo_id: &str,
) -> Result<Vec<HfTreeEntry>, ByoError> {
    let tree_url = format!(
        "https://huggingface.co/api/models/{}/tree/main?recursive=true",
        repo_id
    );
    let response = client.get(&tree_url).send().await.map_err(network_error)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND
        || response.status() == reqwest::StatusCode::UNAUTHORIZED
    {
        return Err(ByoError::new(
            ByoErrorCode::HfRepoNotFound,
            format!("Repo '{}' not found or not public", repo_id),
        ));
    }
    if !response.status().is_success() {
        return Err(ByoError::new(
            ByoErrorCode::NetworkError,
            format!("HuggingFace API returned HTTP {}", response.status()),
        ));
    }
    response.json().await.map_err(network_error)
}

/// One `.onnx` file found in a HuggingFace repo, offered to the frontend as
/// an import candidate when a repo has more than one.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfOnnxCandidate {
    path: String,
    size_bytes: u64,
}

/// List every `.onnx` file in a HuggingFace repo. When `url` already
/// points at one exact file (a `/blob/...` or `/resolve/...` link), returns
/// that single file (validating it actually exists) instead of the whole
/// repo's `.onnx` files - lets the frontend treat "one candidate" the same
/// way whether the user pasted a repo or a direct file link.
#[tauri::command]
pub async fn list_hf_onnx_files(url: String) -> Result<Vec<HfOnnxCandidate>, String> {
    let (repo_id, file_path) = parse_hf_input(&url).map_err(|e| e.to_command_error())?;
    let client = reqwest::Client::new();
    let entries = fetch_hf_tree(&client, &repo_id)
        .await
        .map_err(|e| e.to_command_error())?;
    let onnx_files: Vec<&HfTreeEntry> = entries
        .iter()
        .filter(|e| e.entry_type == "file" && e.path.to_ascii_lowercase().ends_with(".onnx"))
        .collect();

    let candidates = if let Some(path) = file_path {
        match onnx_files.iter().find(|e| e.path == path) {
            Some(entry) => vec![HfOnnxCandidate {
                path: entry.path.clone(),
                size_bytes: entry.size,
            }],
            None => {
                return Err(ByoError::new(
                    ByoErrorCode::HfRepoNotFound,
                    format!("Repo '{}' has no file at '{}'", repo_id, path),
                )
                .to_command_error());
            }
        }
    } else {
        onnx_files
            .iter()
            .map(|e| HfOnnxCandidate {
                path: e.path.clone(),
                size_bytes: e.size,
            })
            .collect()
    };

    if candidates.is_empty() {
        return Err(ByoError::new(
            ByoErrorCode::HfRepoNotFound,
            format!("Repo '{}' has no .onnx model file", repo_id),
        )
        .to_command_error());
    }
    Ok(candidates)
}

/// Map a reqwest failure to a typed network error.
fn network_error(e: impl std::fmt::Display) -> ByoError {
    ByoError::new(ByoErrorCode::NetworkError, format!("Network error: {}", e))
}

/// Import one exact, already-resolved `.onnx` file (plus any known
/// companions) from a HuggingFace repo. `file_path` must be an entry this
/// repo actually has - callers resolve ambiguity via `list_hf_onnx_files`
/// before ever reaching this function, so there is none left to handle here.
async fn import_from_hf(
    app: &AppHandle,
    repo_id: String,
    file_path: String,
) -> Result<CustomModelInfo, ByoError> {
    // Reject an overlapping download (registry or another BYO import) instead
    // of corrupting shared progress state - see DOWNLOAD_SESSION_LOCK.
    let _guard = crate::commands::transcription::acquire_download_lock()
        .await
        .map_err(|e| ByoError::new(ByoErrorCode::DownloadInProgress, e))?;

    if let Some(existing) = find_duplicate_hf(&scan_custom_models(), &repo_id) {
        return Err(duplicate_error(existing));
    }
    let client = reqwest::Client::new();
    let entries = fetch_hf_tree(&client, &repo_id).await?;

    let model_entry = entries
        .iter()
        .find(|e| e.entry_type == "file" && e.path == file_path)
        .ok_or_else(|| {
            ByoError::new(
                ByoErrorCode::HfRepoNotFound,
                format!(
                    "Repo '{}' no longer has a file at '{}' - it may have changed",
                    repo_id, file_path
                ),
            )
        })?;

    // External-data companion published next to the model file, if any.
    let data_path = format!("{}.data", model_entry.path);
    let data_entry = entries
        .iter()
        .find(|e| e.entry_type == "file" && e.path == data_path);

    let total_bytes: u64 = model_entry.size + data_entry.map(|e| e.size).unwrap_or(0);
    if total_bytes > MAX_CUSTOM_MODEL_BYTES {
        return Err(ByoError::new(
            ByoErrorCode::FileTooLarge,
            format!(
                "Model is {} bytes; the limit is {} bytes",
                total_bytes, MAX_CUSTOM_MODEL_BYTES
            ),
        ));
    }

    // Display name = repo name segment; dir name = sanitized form of it.
    let display_name = repo_id
        .split('/')
        .next_back()
        .unwrap_or("custom-model")
        .to_string();
    let model_dir = resumable_model_dir(&display_name)?;
    std::fs::create_dir_all(&model_dir)
        .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;

    let file_url = |repo_path: &str| {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_id, repo_path
        )
    };

    // Same `model-download-start` event registry downloads emit - the
    // page-level `modelDownloadEvents` singleton picks it up so this download
    // is visible in the floating/minimized indicator exactly like a registry
    // download, even after this modal closes.
    let _ = app.emit(
        "model-download-start",
        serde_json::json!({
            "model_id": "custom",
            "display_name": display_name,
            "expected_size_mb": total_bytes / 1_000_000,
            "model_dir_name": model_dir.file_name().and_then(|n| n.to_str()).unwrap_or_default(),
        }),
    );

    // Reuse the registry downloader (`download_single_file`) instead of a
    // bespoke stream - gets resumable downloads (Range + `.partial` file) and
    // minimize-safe progress events for free, matching registry model
    // downloads instead of a from-scratch reimplementation that loses both.
    let mut downloaded: u64 = 0;
    let total_files = if data_entry.is_some() { 2 } else { 1 };
    let import_result: Result<(), ByoError> = async {
        crate::model_download::downloader::download_single_file(
            &client,
            &file_url(&model_entry.path),
            &model_dir.join(MODEL_FILE_NAME),
            app,
            0,
            total_files,
            &mut downloaded,
            total_bytes,
        )
        .await
        .map_err(network_error)?;
        if let Some(data) = data_entry {
            crate::model_download::downloader::download_single_file(
                &client,
                &file_url(&data.path),
                &model_dir.join(MODEL_DATA_FILE_NAME),
                app,
                1,
                total_files,
                &mut downloaded,
                total_bytes,
            )
            .await
            .map_err(network_error)?;
        }
        // Best-effort companions: tokenizer/config make decoding better but
        // their absence is not an error. Fixed local names from our allowlist.
        for companion in COMPANION_FILES {
            if let Some(entry) = entries
                .iter()
                .find(|e| e.entry_type == "file" && e.path == companion)
            {
                let _ = crate::model_download::downloader::download_single_file(
                    &client,
                    &file_url(&entry.path),
                    &model_dir.join(companion),
                    app,
                    total_files,
                    total_files + 1,
                    &mut downloaded,
                    total_bytes + entry.size,
                )
                .await;
            }
        }
        Ok(())
    }
    .await;
    if let Err(e) = import_result {
        cleanup_after_failed_import(&model_dir);
        return Err(e);
    }

    // `download_single_file` doesn't enforce a byte cap mid-stream (registry
    // downloads don't need one - model sizes are pinned in our own manifest).
    // BYO sources are untrusted, so check actual bytes on disk against the
    // declared total as a backstop against a repo that lies about file size.
    let actual_bytes: u64 = std::fs::metadata(model_dir.join(MODEL_FILE_NAME))
        .map(|m| m.len())
        .unwrap_or(0)
        + data_entry
            .map(|_| {
                std::fs::metadata(model_dir.join(MODEL_DATA_FILE_NAME))
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
    if actual_bytes > MAX_CUSTOM_MODEL_BYTES {
        let _ = remove_model_dir(&model_dir);
        return Err(ByoError::new(
            ByoErrorCode::FileTooLarge,
            format!(
                "Downloaded {} bytes; the limit is {} bytes",
                actual_bytes, MAX_CUSTOM_MODEL_BYTES
            ),
        ));
    }

    // Validate on a blocking thread - identical policy to local imports.
    let validate_path = model_dir.join(MODEL_FILE_NAME);
    let validation =
        tauri::async_runtime::spawn_blocking(move || validate_onnx_file(&validate_path))
            .await
            .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()))?;
    if let Err(e) = validation {
        let _ = remove_model_dir(&model_dir);
        return Err(e);
    }

    let size_bytes = std::fs::metadata(model_dir.join(MODEL_FILE_NAME))
        .map(|m| m.len())
        .unwrap_or(model_entry.size);
    let original_filename = model_entry
        .path
        .rsplit('/')
        .next()
        .unwrap_or(MODEL_FILE_NAME)
        .to_string();
    let info = CustomModelInfo {
        name: display_name,
        format: "onnx".to_string(),
        size_bytes,
        source: repo_id.clone(),
        original_filename,
        imported_at: chrono::Utc::now().to_rfc3339(),
        is_loadable: true,
        source_path: None,
    };
    finalize_import(
        &model_dir,
        &info,
        Some(format!("https://huggingface.co/{}", repo_id)),
    )?;
    // Payload must be the model id (a bare string), matching what registry
    // downloads emit - `modelDownloadEvents`'s completion handler reads
    // `event.payload as string` to route `selectModel`/`markModelDownloaded`.
    let _ = app.emit("model-download-complete", "custom");
    Ok(info)
}

/// Check for leftover `.partial` bytes from a cancelled BYO import.
/// `model_dir_name` is the sanitized directory name under `custom-models/`
/// (the value `unique_model_dir` produces) - NOT a registry model id, so this
/// is intentionally a separate command from `check_partial_download`, which
/// only understands the registry's HF-cache directory layout.
#[tauri::command]
pub async fn check_custom_model_partial_download(
    model_dir_name: String,
) -> Result<Option<u64>, String> {
    let dir = custom_models_root()
        .map_err(|e| e.to_command_error())?
        .join(sanitize_model_dir_name(&model_dir_name));
    if !dir.exists() {
        return Ok(None);
    }
    let total = crate::model_download::sum_partial_files(&dir);
    Ok((total > 0).then_some(total))
}

/// Delete a cancelled BYO import's leftover directory (the BYO equivalent of
/// `delete_partial_download`).
#[tauri::command]
pub async fn delete_custom_model_partial_download(model_dir_name: String) -> Result<(), String> {
    let dir = custom_models_root()
        .map_err(|e| e.to_command_error())?
        .join(sanitize_model_dir_name(&model_dir_name));
    remove_model_dir(&dir).map_err(|e| e.to_command_error())
}

/// Absolute path of a BYO model's directory, for the "open download folder"
/// button while a custom-model download is in progress (before the model has
/// a `list_custom_models` entry to read a path from).
#[tauri::command]
pub async fn get_custom_model_dir_path(model_dir_name: String) -> Result<String, String> {
    let dir = custom_models_root()
        .map_err(|e| e.to_command_error())?
        .join(sanitize_model_dir_name(&model_dir_name));
    Ok(dir.to_string_lossy().to_string())
}

/// Import a custom ONNX model from a local file.
#[tauri::command]
pub async fn import_custom_model_from_file(
    source_path: String,
    companion_data_path: Option<String>,
    display_name: String,
) -> Result<CustomModelInfo, String> {
    import_local(source_path, companion_data_path, display_name)
        .await
        .map_err(|e| e.to_command_error())
}

/// Import a custom ONNX model from a HuggingFace repo, given the exact
/// `file_path` within it to import (resolved beforehand via
/// `list_hf_onnx_files` - either the repo's single `.onnx` file or the one
/// the user picked from a list).
#[tauri::command]
pub async fn import_custom_model_from_url(
    app: AppHandle,
    url: String,
    file_path: String,
) -> Result<CustomModelInfo, String> {
    let (repo_id, _) = parse_hf_input(&url).map_err(|e| e.to_command_error())?;
    // Fresh import: clear any stale cancel request from a previous download.
    crate::commands::transcription::DOWNLOAD_CANCELLED
        .store(false, std::sync::atomic::Ordering::SeqCst);
    import_from_hf(&app, repo_id, file_path).await.map_err(|e| {
        // Same distinction registry downloads make: a user-initiated cancel
        // is not an error state for the shared download indicator.
        if crate::commands::transcription::DOWNLOAD_CANCELLED
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            let _ = app.emit("model-download-cancelled", ());
        } else {
            let _ = app.emit("model-download-error", ());
        }
        e.to_command_error()
    })
}

/// Delete one imported custom model by its directory path. If it was the
/// selected custom model, clear the selection; if it was also the active
/// transcription model, fall back to the default.
#[tauri::command]
pub async fn remove_custom_model(path: String) -> Result<(), String> {
    let dir = PathBuf::from(&path);
    remove_model_dir(&dir).map_err(|e| e.to_command_error())?;
    AppSettings::update(|s| {
        if s.custom_model_path.as_deref() == Some(path.as_str()) {
            s.custom_model_path = None;
            s.custom_model_format = None;
            s.custom_model_name = None;
            s.custom_model_url = None;
            if s.model == "custom" {
                s.model = crate::settings::DEFAULT_MODEL_ID.to_string();
            }
        }
    })
    .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()).to_command_error())?;
    Ok(())
}

/// List every imported custom model by scanning the managed root for
/// directories with a readable `meta.json`. Newest imports first.
#[tauri::command]
pub async fn list_custom_models() -> Result<Vec<CustomModelEntry>, String> {
    let mut models = scan_custom_models();
    models.sort_by(|a, b| b.info.imported_at.cmp(&a.info.imported_at));
    Ok(models)
}

/// Point settings at an already-imported model so selecting "custom" in the
/// picker resolves to it. Returns the model's metadata.
#[tauri::command]
pub async fn select_custom_model(path: String) -> Result<CustomModelInfo, String> {
    let dir = verify_managed_model_dir(Path::new(&path)).map_err(|e| e.to_command_error())?;
    let contents = std::fs::read_to_string(dir.join("meta.json")).map_err(|e| {
        ByoError::new(ByoErrorCode::CorruptOrInvalid, e.to_string()).to_command_error()
    })?;
    let info: CustomModelInfo = serde_json::from_str(&contents).map_err(|e| {
        ByoError::new(ByoErrorCode::CorruptOrInvalid, e.to_string()).to_command_error()
    })?;
    let source_url =
        (info.source != "file").then(|| format!("https://huggingface.co/{}", info.source));
    AppSettings::update(|s| {
        s.custom_model_path = Some(path.clone());
        s.custom_model_format = Some(info.format.clone());
        s.custom_model_name = Some(info.name.clone());
        s.custom_model_url = source_url.clone();
    })
    .map_err(|e| ByoError::new(ByoErrorCode::Internal, e.to_string()).to_command_error())?;
    Ok(info)
}

/// Read the imported model's metadata for the Settings UI. Returns `None`
/// when nothing is imported (or the files were removed externally).
#[tauri::command]
pub async fn get_custom_model_info() -> Result<Option<CustomModelInfo>, String> {
    let settings = AppSettings::get();
    let Some(dir) = settings.custom_model_path.as_deref() else {
        return Ok(None);
    };
    let meta_path = Path::new(dir).join("meta.json");
    if !meta_path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&meta_path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<CustomModelInfo>(&contents) {
        Ok(info) => Ok(Some(info)),
        Err(e) => {
            log::warn!("[byo] Unreadable meta.json: {}", e);
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hf_input_accepts_common_repo_forms() {
        assert_eq!(
            parse_hf_input("org/repo").unwrap(),
            ("org/repo".to_string(), None)
        );
        assert_eq!(
            parse_hf_input("https://huggingface.co/org/repo").unwrap(),
            ("org/repo".to_string(), None)
        );
        assert_eq!(
            parse_hf_input("https://huggingface.co/org/repo/tree/main").unwrap(),
            ("org/repo".to_string(), None)
        );
        assert_eq!(
            parse_hf_input(" org/repo/ ").unwrap(),
            ("org/repo".to_string(), None)
        );
    }

    #[test]
    fn parse_hf_input_extracts_direct_file_links() {
        assert_eq!(
            parse_hf_input("https://huggingface.co/org/repo/blob/main/model.onnx").unwrap(),
            ("org/repo".to_string(), Some("model.onnx".to_string()))
        );
        assert_eq!(
            parse_hf_input("https://huggingface.co/org/repo/resolve/main/onnx/model.onnx").unwrap(),
            ("org/repo".to_string(), Some("onnx/model.onnx".to_string()))
        );
        // A blob link with nothing past the revision has no file path.
        assert_eq!(
            parse_hf_input("https://huggingface.co/org/repo/blob/main").unwrap(),
            ("org/repo".to_string(), None)
        );
    }

    #[test]
    fn parse_hf_input_rejects_garbage() {
        assert!(parse_hf_input("not a repo").is_err());
        assert!(parse_hf_input("https://evil.example/org/repo").is_err());
        assert!(parse_hf_input("org/../repo").is_err());
        assert!(
            parse_hf_input("https://huggingface.co/org/repo/blob/main/../../etc/passwd").is_err()
        );
        assert!(parse_hf_input("https://huggingface.co/org/repo/blob/main/not-onnx.txt").is_err());
    }

    fn entry(name: &str, source: &str, source_path: Option<&str>) -> CustomModelEntry {
        CustomModelEntry {
            info: CustomModelInfo {
                name: name.to_string(),
                format: "onnx".to_string(),
                size_bytes: 1,
                source: source.to_string(),
                original_filename: "model.onnx".to_string(),
                imported_at: "2026-01-01T00:00:00Z".to_string(),
                is_loadable: true,
                source_path: source_path.map(|p| p.to_string()),
            },
            path: format!("/root/{}", name),
        }
    }

    #[test]
    fn duplicate_local_matches_same_canonical_path_only() {
        let models = vec![
            entry("a", "file", Some("/home/u/models/a.onnx")),
            entry("b", "file", None), // pre-source_path meta.json: never a duplicate
        ];
        let dup = find_duplicate_local(&models, "/home/u/models/a.onnx");
        assert_eq!(dup.map(|m| m.info.name.as_str()), Some("a"));
        // Different path (even if same content) imports fine - path-based only.
        assert!(find_duplicate_local(&models, "/home/u/other/a.onnx").is_none());
    }

    #[test]
    fn duplicate_hf_matches_exact_repo_id() {
        let models = vec![entry("p", "org/parakeet", None)];
        assert!(find_duplicate_hf(&models, "org/parakeet").is_some());
        assert!(find_duplicate_hf(&models, "org/other").is_none());
        let e = duplicate_error(&models[0]);
        assert_eq!(e.code, ByoErrorCode::DuplicateSource);
        assert!(e.message.contains("'p'"));
    }

    #[test]
    fn meta_json_without_source_path_still_deserializes() {
        let legacy = r#"{
            "name": "old",
            "format": "onnx",
            "sizeBytes": 5,
            "source": "file",
            "originalFilename": "old.onnx",
            "importedAt": "2026-01-01T00:00:00Z",
            "isLoadable": true
        }"#;
        let info: CustomModelInfo = serde_json::from_str(legacy).unwrap();
        assert_eq!(info.source_path, None);
    }
}
