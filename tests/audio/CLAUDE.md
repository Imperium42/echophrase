# tests/audio - Audio Test Fixtures

## Fixtures

| File | Spec | Content | Used by |
|------|------|---------|---------|
| `test-1-2.wav` | 16 kHz, mono, 16-bit, ~2s, quiet (peak ~0.055) | A voice saying "Test one, two." | `test_transcribe_audio_file` (commands/transcription.rs), `transcription/tests.rs`, `minimal_test.rs` |
| `mandarin-sample.wav` *(local-only, not committed)* | 16 kHz, mono, 16-bit, ~4s | Any Mandarin sentence (e.g. TTS: 我想要一杯咖啡，谢谢。) | `qwen3_asr` `e2e_mandarin_fixture_s2t` (skips when absent). Generate: `edge-tts --voice zh-CN-XiaoxiaoNeural --text ... --write-media out.mp3` then `ffmpeg -i out.mp3 -ar 16000 -ac 1 -sample_fmt s16 mandarin-sample.wav` |

`test-1-2.wav` is the maintainer's (Dylan Hunt's) own voice, recorded for this
project and published with their consent under the repo license.

Expected transcript: **"Test one, two."** - if a pipeline change garbles this,
the change broke preprocessing (resample/downmix/normalize) or decoding.

The file is intentionally QUIET: it exercises peak normalization (the pipeline
normalizes to 0.8 peak via `echo_audio`). Don't replace it with a loud take.

## Running the fixture-based tests

```bash
# Real end-to-end transcription (ignored by default - loads the actual model):
cargo test -p echophrase test_transcribe_audio_file --lib -- --ignored --nocapture
# Platform expectation: CoreML on macOS, DirectML on Windows, CPU on Linux
# (see the per-platform assertions in commands/transcription.rs).
```

## Hands-free LIVE E2E (no fixture - TTS through the speakers)

For verifying the real capture stack (mic -> in-memory buffer -> transcribe ->
paste) without a human speaking, drive the app with the record hotkey and
macOS text-to-speech:

```bash
# App must be running (bun run tauri:dev:mac). Terminal needs accessibility +
# the mic must be able to hear the speakers.
osascript -e 'tell application "System Events" to key code 105'  # F13: start
sleep 1
say -r 170 "Testing the unified in memory audio pipeline, one two three"
sleep 1
osascript -e 'tell application "System Events" to key code 105'  # F13: stop
sleep 5
pbpaste  # transcript lands on the clipboard via paste-at-cursor
```

Why this works: `key code 105` is F13 (the remapped Dictation key = default mac
record hotkey), and `say` produces real acoustic audio the microphone captures,
so the whole cpal/CoreAudio -> 16 kHz mono buffer -> model -> output path is
exercised exactly as a user would. Verify in-memory capture by confirming no
new `*.wav` files appear during the cycle.
