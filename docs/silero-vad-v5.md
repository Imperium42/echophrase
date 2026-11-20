# Silero VAD V5 - Implementation Notes

Critical implementation details for the Silero VAD V5 ONNX model used in Open Mic mode. These were discovered through debugging in April 2026 and are not well-documented upstream.

## Model Source

- Repository: [snakers4/silero-vad](https://github.com/snakers4/silero-vad)
- Model file: `src-tauri/resources/silero_vad.onnx` (bundled at compile time via `include_bytes!`)
- Version: V5 (current as of 2026)

## ONNX Interface (V5 vs V4 - Breaking Change)

Silero VAD V5 changed its ONNX tensor names from V4. Using V4 names causes `Invalid input name: "h"` errors on every inference, returning probability 0.0.

| | V4 (old) | V5 (current) |
|---|---|---|
| **State input** | `"h"` [2, 1, 64] + `"c"` [2, 1, 64] | `"state"` [2, 1, 128] |
| **State output** | `"hn"` + `"cn"` | `"stateN"` |
| **Audio input** | `"input"` [1, N] | `"input"` [1, N] (unchanged) |
| **Sample rate** | `"sr"` [1] | `"sr"` [] (scalar) |
| **Probability output** | `"output"` | `"output"` (unchanged) |

Both scalar `sr` (shape `[]`) and array `sr` (shape `[1]`) are accepted for the sample rate input.

## Chunk Size - CRITICAL

**Must use 256-sample chunks (16ms at 16kHz). Do NOT use 512 samples.**

The model's internal ONNX graph uses conditional `If` nodes that branch based on chunk size. With 512-sample chunks, the LSTM state tensor accumulates incorrect dimensions after the first prediction:

- Expected by LSTM: `{1, 1, 128}` (3 dimensions)
- Actual after 512-sample chunk: `{1, 1, 128, 2}` (4 dimensions)

This causes `Input X must have 3 dimensions only` errors in subsequent frames. The model swallows these errors and returns `prob=0.0006` for all inputs, regardless of audio content.

**Test results (same audio, different chunk sizes):**

| Chunk size | Samples | Duration | Speech prob | Status |
|---|---|---|---|---|
| 256 | 256 | 16ms | **0.35** | Works correctly |
| 512 | 512 | 32ms | 0.0006 | LSTM state corruption |
| 768 | 768 | 48ms | CRASH | InvalidArgument |
| 1024 | 1024 | 64ms | CRASH | InvalidArgument |
| 1536 | 1536 | 96ms | CRASH | InvalidArgument |

This behavior was tested with both the bundled model and the official model downloaded from GitHub - both exhibit the same issue. Likely an ORT 1.23+/1.24 incompatibility with the model's `If` node graph structure at chunk sizes != 256.

## Audio Normalization for WSLg

WSLg's `RDPSource` PulseAudio source captures audio at very low levels (~-38dB, peak ~0.01 f32). Normal speech peaks at 0.1-0.5. Whisper normalizes audio internally so push-to-talk transcription works fine, but the Silero VAD model expects normal amplitude levels.

**Solution:** Before feeding audio to `vad.predict()`, normalize samples to peak 0.5:

```rust
let peak = samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
let vad_samples = if peak > 0.0001 {
    let gain = (0.5 / peak).min(100.0);
    samples.iter().map(|s| (s * gain).clamp(-1.0, 1.0)).collect()
} else {
    samples.clone()
};
let probability = vad.predict(&vad_samples);
```

The gain cap of 100x prevents amplifying pure noise/silence. The raw (unnormalized) buffer is preserved for Whisper transcription.

## State Management

- State must be reset between recording sessions (`vad.reset()` zeros the `[2, 1, 128]` tensor)
- State is stateful across chunks within a session - do NOT reset between chunks
- After emitting a completed speech segment (`open-mic-chunk-ready`), reset state for the next utterance

## Speech Detection Thresholds

| Parameter | Default | Range | Setting key |
|---|---|---|---|
| VAD threshold | 0.5 (50%) | 0.0 - 1.0 | `voice_threshold` (0-100 in UI) |
| Silence timeout | 5s | 2-10s | `silence_timeout` |
| Min speech duration | 500ms | hardcoded | `MIN_SPEECH_DURATION_MS` |

- **VAD threshold**: `probability >= threshold` determines speech/non-speech per chunk
- **Silence timeout**: How long silence must persist after speech before triggering transcription
- **Min speech duration**: Wall-clock time from speech onset; shorter = noise, discarded

## Key Files

- `src-tauri/src/vad.rs` - SileroVad struct, VadBackend trait, predict/reset
- `src-tauri/src/audio/recorder.rs` - VAD recording loop, state machine, buffer management
- `src-tauri/src/commands/recording.rs` - Tauri commands, RMS silence guard
- `src-tauri/resources/silero_vad.onnx` - Bundled model file

## Debugging

Set `OPEN_MIC_DEBUG = true` in `recorder.rs` to enable per-frame logging:

```
[OpenMic] frame=100 prob=0.994 rms=0.03792 peak=0.0791 speech=true state=Speaking
```

Fields: frame count, VAD probability (0-1), raw audio RMS, raw audio peak, speech classification, state machine state.
