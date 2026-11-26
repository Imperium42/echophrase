# echophrase-cli TODO

Future-broadening work. The inline `// TODO:` comments in the source are the
source of truth; this file just aggregates them for discoverability. Keep the two
in sync when you add or resolve an item.

## Broader file-ingest (`transcode`)

- **Compressed input formats** (`src/recorder.rs`, `read_wav_to_mono_f32`):
  WAV-only today. Broaden to mp3/m4a/flac/ogg via `symphonia` so users can
  transcode arbitrary voice files, not just WAV.
- **Higher-fidelity resampling** (`src/recorder.rs`, `resample_linear`):
  linear interpolation is fine for speech; consider a sinc/`rubato`-based
  resampler for higher-quality downsampling.
- **Proper multi-channel downmix** (`src/recorder.rs`, `downmix_to_mono`):
  naive channel averaging; consider proper downmix weighting (e.g. center/LFE
  coefficients) for >2 channel sources.
- **Batch / glob / directory input** (`src/commands/transcode.rs`, `run`):
  single-file only today. Broaden to batch / glob / directory input and a
  `--recursive` mode.
- **Configurable output format** (`src/commands/transcode.rs`, output handling):
  only emits 16 kHz mono 16-bit WAV. Broaden to configurable
  rate/bit-depth/format if non-pipeline consumers need it.
- **Streaming I/O** (`src/cli.rs`, `TranscodeArgs`): add `--stdin`/`--stdout`
  streaming (and an MCP tool for it) so audio can be piped without temp files.
- ~~**Transcription tool**~~: DONE. `echophrase-cli transcribe <file.wav>` and the
  MCP `transcribe` tool call the running desktop app over a localhost-only
  IPC connection (`src-tauri/src/ipc/`), reusing its already-loaded Candle
  Whisper model. Requires the desktop app to be running.
