// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

use super::candle_whisper;
use std::env;

#[tokio::test]
#[ignore = "Requires CUDA/GPU - run with: cargo test -- --ignored"]
async fn test_transcribe_test_one_two() {
    // Skip if no model directory and offline
    if env::var("WHISPER_MODEL_DIR").is_err() && env::var("OFFLINE_TESTS").is_ok() {
        eprintln!("Skipping test: WHISPER_MODEL_DIR not set and OFFLINE_TESTS is set");
        return;
    }

    // Test audio path (absolute from repo root)
    let repo_root = env::current_dir().unwrap().parent().unwrap().to_path_buf();
    let test_audio = repo_root.join("tests/audio/test-1-2.wav");
    let test_audio = test_audio.to_str().unwrap();

    // Run transcription
    match candle_whisper::transcribe_audio_file(test_audio).await {
        Ok(transcript) => {
            println!("Transcript: '{}'", transcript);
            let lower = transcript.to_lowercase();

            // Accept various formats: "test one two", "test 1 2", etc
            assert!(
                (lower.contains("test") && lower.contains("one") && lower.contains("two"))
                    || (lower.contains("test") && lower.contains("1") && lower.contains("2")),
                "Expected 'test one two' or 'test 1 2', got: '{}'",
                transcript
            );
        }
        Err(e) => {
            // If model download fails in CI, skip test
            if e.to_string().contains("Failed to download model") {
                eprintln!("Skipping test due to model download failure: {}", e);
                return;
            }
            panic!("Transcription failed: {}", e);
        }
    }
}

#[test]
fn test_read_wav_file() {
    // TODO: Test WAV file reading functionality
    // TODO: Test normalization
    // TODO: Test resampling if implemented

    // Skip for now
    println!("Test not implemented yet");
}

#[test]
fn test_model_path_resolution() {
    // TODO: Test model path finding logic
    // TODO: Test fallback behavior

    // Skip for now
    println!("Test not implemented yet");
}

/// Integration test that actually transcribes audio
#[tokio::test]
async fn test_full_transcription_pipeline() {
    // TODO: Skip if no model available
    // TODO: Load test audio file
    // TODO: Run full transcription
    // TODO: Verify output

    if env::var("WHISPER_TEST_MODEL_DIR").is_err() {
        eprintln!("Skipping integration test: WHISPER_TEST_MODEL_DIR not set");
        return;
    }

    let audio_path = "../../tests/audio/test-1-2.wav";

    // TODO: Run transcription
    let result = candle_whisper::transcribe_audio_file(audio_path).await;

    assert!(result.is_err(), "Expected error since not implemented");
}
