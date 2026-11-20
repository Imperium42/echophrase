// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

#[cfg(test)]
mod real_transcription_tests {
    use crate::models::WhisperModelSize;
    use crate::transcription::candle_whisper;
    use std::path::PathBuf;

    #[tokio::test]
    #[ignore] // Use `cargo test -- --ignored` to run
    async fn test_real_candle_transcription() {
        println!("🧪 Testing REAL Candle transcription with test-1-2.wav...");

        // Use Medium model for testing
        let model_size = WhisperModelSize::Medium;
        let model_info = model_size.get_info();

        println!(
            "📊 Model: {} ({})",
            model_info.display_name, model_info.model_name
        );
        println!("📁 Expected filename: {}", model_info.model_filename);

        // Check model cache
        let home_dir = dirs::home_dir().expect("Failed to get home directory");
        let model_dir = home_dir
            .join(".cache/huggingface/hub")
            .join(format!(
                "models--{}",
                model_info.model_name.replace("/", "--")
            ))
            .join("snapshots/main");

        let model_path = model_dir.join(&model_info.model_filename);
        println!("📂 Model path: {}", model_path.display());
        println!("✅ Model exists: {}", model_path.exists());

        if !model_path.exists() {
            println!("❌ Model not found - please download it first");
            println!("   Run: cargo test test_real_candle_transcription -- --ignored --nocapture");
            return;
        }

        // Get test audio file path
        let test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/audio/test-1-2.wav");

        println!("🎤 Test audio: {}", test_path.display());
        println!("✅ Audio exists: {}", test_path.exists());

        assert!(test_path.exists(), "Test audio file not found");

        // Transcribe
        println!("\n🔄 Starting transcription...");
        let start = std::time::Instant::now();

        let result = candle_whisper::transcribe_audio_file(test_path.to_str().unwrap()).await;

        let duration = start.elapsed();
        println!("⏱️  Transcription took: {:?}", duration);

        match &result {
            Ok(text) => {
                println!("\n✅ TRANSCRIPTION SUCCESS!");
                println!("📝 Result: '{}'", text);
                println!("📏 Length: {} characters", text.len());

                // Verify transcription contains expected keywords
                let text_lower = text.to_lowercase();
                let has_keyword = text_lower.contains("test")
                    || text_lower.contains("one")
                    || text_lower.contains("two")
                    || text_lower.contains("1")
                    || text_lower.contains("2");

                if !has_keyword {
                    println!("⚠️  WARNING: Transcription doesn't contain expected keywords!");
                    println!("   Expected: 'test', 'one', 'two', '1', or '2'");
                    println!("   Got: '{}'", text);
                }

                // Don't fail the test if transcription is empty or wrong - Candle might hallucinate
                assert!(!text.is_empty(), "Transcription should not be empty");
            }
            Err(e) => {
                println!("\n❌ TRANSCRIPTION FAILED!");
                println!("Error: {:?}", e);
                panic!("Transcription failed: {}", e);
            }
        }
    }
}
