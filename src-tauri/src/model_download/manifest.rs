// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

//! Pinned SHA256 digests for first-party model files.
//!
//! Model binaries are downloaded over HTTPS and then mmap-parsed (safetensors)
//! or fed to ONNX Runtime, so a tampered or corrupted download is a native
//! code-adjacent attack surface. Every LFS-tracked binary in the registry is
//! pinned here; a freshly downloaded file whose hash doesn't match is deleted
//! and the download fails loudly.
//!
//! Small non-LFS text files (config.json, vocab.txt, tokenizer configs) are
//! not pinned - HuggingFace's tree API only exposes SHA256 for LFS objects,
//! and these files are plain-text parsed, not mmap'd. User-supplied
//! `custom_model_url` downloads are intentionally exempt: bringing your own
//! model is an explicit opt-in (mirrors LM Studio / Ollama BYO-model policy).
//!
//! Digests sourced from `https://huggingface.co/api/models/<repo>/tree/main`
//! (LFS oid = SHA256) on 2026-07-03. When a registry model is added or its
//! revision changes, refresh the pins from the same API.

use sha2::{Digest, Sha256};
use std::io::Read;

/// (repo_id, file_path, sha256) pins for registry model files.
const PINNED_DIGESTS: &[(&str, &str, &str)] = &[
    // parakeet-tdt-0.6b-v3
    (
        "istupakov/parakeet-tdt-0.6b-v3-onnx",
        "encoder-model.onnx",
        "98a74b21b4cc0017c1e7030319a4a96f4a9506e50f0708f3a516d02a77c96bb1",
    ),
    (
        "istupakov/parakeet-tdt-0.6b-v3-onnx",
        "encoder-model.onnx.data",
        "9a22d372c51455c34f13405da2520baefb7125bd16981397561423ed32d24f36",
    ),
    (
        "istupakov/parakeet-tdt-0.6b-v3-onnx",
        "decoder_joint-model.onnx",
        "e978ddf6688527182c10fde2eb4b83068421648985ef23f7a86be732be8706c1",
    ),
    (
        "istupakov/parakeet-tdt-0.6b-v3-onnx",
        "nemo128.onnx",
        "a9fde1486ebfcc08f328d75ad4610c67835fea58c73ba57e3209a6f6cf019e9f",
    ),
    // parakeet-eou-120m
    (
        "altunenes/parakeet-rs",
        "realtime_eou_120m-v1-onnx/encoder.onnx",
        "d472887cc38a784a5bfc21c2dbe247639edc3b3f9992388d8ceceaec07256b5b",
    ),
    (
        "altunenes/parakeet-rs",
        "realtime_eou_120m-v1-onnx/decoder_joint.onnx",
        "9d2553ac043c2fc5f69e970769b0fb8ab9103fbfdeb7d26a1ea9729d4bd2dddd",
    ),
    // qwen3-asr-0.6b (digests from HF tree API, 2026-07-19; tokenizer.json is
    // LFS-tracked in this repo so it gets a pin too)
    (
        "andrewleech/qwen3-asr-0.6b-onnx",
        "encoder.int4.onnx",
        "3c027f880f677615de85e1f6934906e1d5d77624b724096d33accef99c753eed",
    ),
    (
        "andrewleech/qwen3-asr-0.6b-onnx",
        "decoder_init.int4.onnx",
        "6d54633bbb9e6b1ecf372f218d04adb3bf6f2bfc442ce55c988526d532896faa",
    ),
    (
        "andrewleech/qwen3-asr-0.6b-onnx",
        "decoder_step.int4.onnx",
        "7eaa4f31d5eb6ae2937ff45222c32f692f2d5ac29b46cbae9046da332a9f317d",
    ),
    (
        "andrewleech/qwen3-asr-0.6b-onnx",
        "decoder_weights.int4.data",
        "d68cd1c0695a7ba42651d06b1bc1158e2e58af5f9adbb6dba874fbbb8a4f22cf",
    ),
    (
        "andrewleech/qwen3-asr-0.6b-onnx",
        "embed_tokens.bin",
        "e80150119fa5f7e56e85aed64c3a02d5c78eb7a37cfdcb973d0987316f15bee2",
    ),
    (
        "andrewleech/qwen3-asr-0.6b-onnx",
        "tokenizer.json",
        "bd2a97b55c8f7f9c328c73ee9b9178771037e9f566dfca8e238a063d41cbac92",
    ),
    // t5-mini-grammar
    (
        "visheratin/t5-efficient-mini-grammar-correction",
        "encoder_model_quantized.onnx",
        "67bb9f9b0db9a50eca338e6297a15b30c73042b3292b42b113c10df882140a24",
    ),
    (
        "visheratin/t5-efficient-mini-grammar-correction",
        "init_decoder_model_quantized.onnx",
        "d11979f42ce0aaad68e6b59a5583632968dbe1bfebab713467c0dc2b5f079fe9",
    ),
];

/// Look up the pinned SHA256 for a registry model file, if one exists.
pub fn pinned_sha256(repo_id: &str, file_path: &str) -> Option<&'static str> {
    PINNED_DIGESTS
        .iter()
        .find(|(repo, file, _)| *repo == repo_id && *file == file_path)
        .map(|(_, _, digest)| *digest)
}

/// Streams `path` through SHA256 and compares it against the pinned digest
/// for `(repo_id, file_path)`. No-op `Ok` when the file has no pin. On a
/// mismatch the file is deleted (so a retry re-downloads) and an error is
/// returned. Runs on a blocking thread - model files can be multi-GB.
pub async fn verify_pinned_file(
    repo_id: &str,
    file_path: &str,
    on_disk: &std::path::Path,
) -> Result<(), String> {
    let Some(expected) = pinned_sha256(repo_id, file_path) else {
        return Ok(());
    };

    let disk_path = on_disk.to_path_buf();
    let actual = tokio::task::spawn_blocking(move || hash_file(&disk_path))
        .await
        .map_err(|e| format!("hash task join error: {e}"))??;

    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        let _ = std::fs::remove_file(on_disk);
        Err(format!(
            "SHA256 mismatch for {file_path}: expected {expected} got {actual}. \
             The download may be corrupted or tampered with - it has been removed, please retry."
        ))
    }
}

/// SHA256 of a file, streamed in 64KB chunks.
fn hash_file(path: &std::path::Path) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("open {} for hash: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("read {} for hash: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_registry_binary_has_a_pin() {
        for entry in crate::models::MODEL_REGISTRY {
            for file in entry.model_files {
                let is_binary = file.ends_with(".onnx")
                    || file.ends_with(".data")
                    || file.ends_with(".bin")
                    || file.ends_with(".safetensors");
                if is_binary {
                    assert!(
                        pinned_sha256(entry.repo_id, file).is_some(),
                        "registry binary {}/{} has no pinned SHA256 - add it to PINNED_DIGESTS",
                        entry.repo_id,
                        file
                    );
                }
            }
        }
    }

    #[test]
    fn lookup_misses_return_none() {
        assert!(pinned_sha256("unknown/repo", "model.onnx").is_none());
        assert!(pinned_sha256("istupakov/parakeet-tdt-0.6b-v3-onnx", "config.json").is_none());
    }

    #[tokio::test]
    async fn verify_rejects_and_deletes_tampered_file() {
        let dir = std::env::temp_dir().join("echophrase-test-manifest");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("encoder-model.onnx");
        std::fs::write(&path, b"tampered bytes").unwrap();

        let result = verify_pinned_file(
            "istupakov/parakeet-tdt-0.6b-v3-onnx",
            "encoder-model.onnx",
            &path,
        )
        .await;

        assert!(result.is_err());
        assert!(!path.exists(), "tampered file must be deleted");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn verify_skips_unpinned_files() {
        let dir = std::env::temp_dir().join("echophrase-test-manifest-unpinned");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("config.json");
        std::fs::write(&path, b"{}").unwrap();

        let result =
            verify_pinned_file("istupakov/parakeet-tdt-0.6b-v3-onnx", "config.json", &path).await;

        assert!(result.is_ok());
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
