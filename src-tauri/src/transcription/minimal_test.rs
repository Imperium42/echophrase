// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

// Minimal Candle Whisper test - EXACT copy of official example decoder logic
// This bypasses all our wrapper code to test the core Candle functionality

use anyhow::Result;
use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as m, Config, audio};
use hound::WavReader;

/// Locate a downloaded `openai/whisper-medium` snapshot in the Hugging Face cache.
/// Override with `WHISPER_MEDIUM_DIR`; otherwise uses `$HF_HOME/hub` or `~/.cache/huggingface/hub`.
fn whisper_medium_snapshot() -> Result<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("WHISPER_MEDIUM_DIR") {
        return Ok(dir.into());
    }
    let hub = std::env::var("HF_HOME")
        .map(|h| std::path::PathBuf::from(h).join("hub"))
        .or_else(|_| {
            dirs::home_dir()
                .map(|h| h.join(".cache/huggingface/hub"))
                .ok_or(std::env::VarError::NotPresent)
        })?;
    let snapshots = hub.join("models--openai--whisper-medium/snapshots");
    std::fs::read_dir(&snapshots)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.join("config.json").exists())
        .ok_or_else(|| anyhow::anyhow!("no whisper-medium snapshot under {}", snapshots.display()))
}

#[test]
#[ignore]
fn test_minimal_candle_whisper() -> Result<()> {
    println!("\n🧪 MINIMAL Candle Whisper Test (EXACT Official Decoder)");

    let audio_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/audio/test-1-2.wav");
    let audio_path = audio_path.display().to_string();
    println!("🎤 Audio: {}", audio_path);

    // 1. Load audio (minimal - just read the WAV)
    let mut reader = WavReader::open(audio_path)?;
    let spec = reader.spec();
    println!(
        "📊 WAV spec: {} channels, {} Hz, {} bits",
        spec.channels, spec.sample_rate, spec.bits_per_sample
    );

    let pcm_data: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap() as f32 / 32768.0)
        .collect();
    println!("📈 Loaded {} samples", pcm_data.len());
    println!(
        "🔍 PCM stats (NO NORMALIZATION): first 5 values: {:?}",
        &pcm_data[..5.min(pcm_data.len())]
    );
    let pcm_max = pcm_data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    println!(
        "🔍 PCM peak: {:.6} ({:.1} dBFS)",
        pcm_max,
        20.0 * pcm_max.log10()
    );

    // SKIP NORMALIZATION - test if official example works without it

    // Verify 16kHz mono
    assert_eq!(spec.sample_rate, 16000, "Must be 16kHz");
    assert_eq!(spec.channels, 1, "Must be mono");

    // 2. Load mel filters (directly from bytes, like official example)
    let mel_bytes = include_bytes!("../../../mel_filters.bytes");
    let mel_bytes_len = (mel_bytes.len() / 4) * 4;
    let mel_bytes = &mel_bytes[..mel_bytes_len];
    let mut mel_filters = vec![0f32; mel_bytes_len / 4];
    <byteorder::LittleEndian as byteorder::ByteOrder>::read_f32_into(mel_bytes, &mut mel_filters);
    println!("🎵 Loaded {} mel filter coefficients", mel_filters.len());

    // 3. Create mel spectrogram (using Candle's audio module directly)
    #[cfg(feature = "cuda")]
    let device = Device::new_cuda(0)?;
    #[cfg(not(feature = "cuda"))]
    let device = Device::Cpu;
    println!("🎮 Device created: {:?}", device);

    let model_path = whisper_medium_snapshot()?;
    println!("📂 Model path: {}", model_path.display());

    // Load config
    let config_path = model_path.join("config.json");
    let config: Config = serde_json::from_str(&std::fs::read_to_string(config_path)?)?;
    println!("⚙️  Config loaded: {} mel bins", config.num_mel_bins);

    // Convert to mel (EXACT official example)
    let mel_vec = audio::pcm_to_mel(&config, &pcm_data, &mel_filters);
    let mel_len = mel_vec.len();
    println!(
        "🔍 Mel vector stats: len={}, first 5 values: {:?}",
        mel_len,
        &mel_vec[..5.min(mel_len)]
    );
    let mel = Tensor::from_vec(
        mel_vec,
        (1, config.num_mel_bins, mel_len / config.num_mel_bins),
        &device,
    )?;
    println!("✅ Mel spectrogram: {:?}", mel.dims());

    // 4. Load model (EXACT official example)
    let model_file = model_path.join("model.safetensors");
    let vb = unsafe { VarBuilder::from_mmaped_safetensors(&[model_file], m::DTYPE, &device)? };
    let mut model = m::model::Whisper::load(&vb, config.clone())?;
    println!("✅ Model loaded");

    // 5. Load tokenizer (EXACT official example)
    let tokenizer_path = model_path.join("tokenizer.json");
    let tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path).map_err(anyhow::Error::msg)?;
    println!("✅ Tokenizer loaded");

    // 6. Create suppress_tokens tensor (EXACT official example lines 123-134)
    let no_timestamps_token = tokenizer.token_to_id(m::NO_TIMESTAMPS_TOKEN).unwrap();
    let timestamps = false; // Use no-timestamps mode for short audio clips
    let suppress_tokens: Vec<f32> = (0..config.vocab_size as u32)
        .map(|i| {
            if config.suppress_tokens.contains(&i) || (timestamps && i == no_timestamps_token) {
                f32::NEG_INFINITY
            } else {
                0f32
            }
        })
        .collect();
    let suppress_tokens_tensor = Tensor::new(suppress_tokens.as_slice(), &device)?;
    println!(
        "🚫 Suppress tokens created ({} suppressed)",
        config.suppress_tokens.len()
    );

    // 7. Encode audio (EXACT official example line 166)
    let audio_features = model.encoder.forward(&mel, true)?;
    println!("🎵 Audio features: {:?}", audio_features.dims());

    // 8. Initialize tokens (EXACT official example lines 173-183)
    let sot_token = tokenizer.token_to_id(m::SOT_TOKEN).unwrap();
    let eot_token = tokenizer.token_to_id(m::EOT_TOKEN).unwrap();
    let language_token = tokenizer.token_to_id("<|en|>").unwrap(); // Force English
    let transcribe_token = tokenizer.token_to_id(m::TRANSCRIBE_TOKEN).unwrap();

    // Initial sequence: [sot, en, transcribe, no_timestamps]
    let mut tokens = vec![sot_token, language_token, transcribe_token];
    if !timestamps {
        tokens.push(no_timestamps_token);
    }
    println!("🎬 Initial tokens: {:?}", tokens);

    // 9. Decoder loop (EXACT official example lines 184-242, greedy only)
    let sample_len = config.max_target_positions / 2; // 224
    for i in 0..sample_len {
        let tokens_t = Tensor::new(tokens.as_slice(), &device)?;
        let tokens_t = tokens_t.unsqueeze(0)?;
        let ys = model.decoder.forward(&tokens_t, &audio_features, i == 0)?;

        let (_, seq_len, _) = ys.dims3()?;
        let logits = model
            .decoder
            .final_linear(&ys.i((..1, seq_len - 1..))?)?
            .i(0)?
            .i(0)?;

        // NOTE: Skipping timestamp rules since timestamps=false

        // Apply suppress_tokens (EXACT official example line 217)
        let logits = logits.broadcast_add(&suppress_tokens_tensor)?;

        // Greedy decoding (EXACT official example lines 224-230)
        let logits_v: Vec<f32> = logits.to_vec1()?;

        // DEBUG: Print top 10 tokens on first iteration
        if i == 0 {
            let mut indexed_logits: Vec<(usize, f32)> =
                logits_v.iter().enumerate().map(|(i, &v)| (i, v)).collect();
            indexed_logits.sort_by(|a, b| b.1.total_cmp(&a.1));
            println!("\n🔍 Top 10 logits:");
            for (idx, (token_id, logit_val)) in indexed_logits.iter().take(10).enumerate() {
                let token_str = tokenizer
                    .decode(&[*token_id as u32], false)
                    .unwrap_or_default();
                println!(
                    "  {}. Token {}: {} = '{}'",
                    idx + 1,
                    token_id,
                    logit_val,
                    token_str
                );
            }
            println!();
        }

        let next_token = logits_v
            .iter()
            .enumerate()
            .max_by(|(_, u), (_, v)| u.total_cmp(v))
            .map(|(i, _)| i as u32)
            .unwrap();

        if i < 5 {
            println!(
                "🔍 Step {}: token = {} ('{}')",
                i,
                next_token,
                tokenizer.decode(&[next_token], false).unwrap_or_default()
            );
        }

        tokens.push(next_token);

        // Check termination (EXACT official example lines 236-240)
        if next_token == eot_token || tokens.len() > config.max_target_positions {
            println!("✅ Hit EOT at step {}", i);
            break;
        }
    }

    // Decode (skip special tokens: sot, en, transcribe, no_timestamps)
    let decode_start = if timestamps { 3 } else { 4 }; // Skip initial tokens

    println!("\n🔍 All generated tokens: {:?}", tokens);
    println!(
        "🔍 Tokens to decode (from index {}): {:?}",
        decode_start,
        &tokens[decode_start..]
    );

    let text = tokenizer
        .decode(&tokens[decode_start..], true)
        .map_err(anyhow::Error::msg)?;
    println!("\n📝 RESULT: '{}'", text);
    println!("📏 {} characters\n", text.len());

    // Check result
    assert!(!text.is_empty(), "Text should not be empty");
    assert!(
        text.to_lowercase().contains("test")
            || text.to_lowercase().contains("one")
            || text.to_lowercase().contains("two"),
        "Should contain 'test', 'one', or 'two', got: '{}'",
        text
    );

    Ok(())
}
