# Echophrase CLI

A small, modern, cross-platform command-line helper for capturing microphone
audio and normalizing audio files - the terminal-native companion to the
Echophrase desktop app.

It **records** the mic (`record`/`stop`/`status`) and **transcodes** existing
WAV files into the canonical format. It also runs as a **minimal MCP server**
(`echophrase-cli mcp`) that exposes those same actions as tools, so AI agents like
Claude Code can drive voice capture without rewriting the audio or session
logic.

- Cross-platform capture via [`cpal`] (WASAPI on Windows, CoreAudio on macOS,
  ALSA on Linux).
- Records and transcodes to **16 kHz, 16-bit, mono** WAV - the exact format the
  Echophrase transcription pipeline expects (captured at the device rate,
  resampled to 16 kHz).
- Background-by-default: `record` returns immediately; `stop` finalizes the file.
- Zero global state: everything lives under one per-user directory you can
  override with `ECHOPHRASE_HOME`. `ECHOPHRASE_IPC_PORT` overrides the port `transcribe` calls
  (default 1424, the desktop app's port; for tests and dev setups).

> Scope note: transcription itself still runs inside the desktop app's Rust
> backend (it owns the loaded model). `echophrase-cli transcribe` and the MCP
> `transcribe` tool talk to the running app over a localhost-only IPC
> connection - the desktop app must be running for either to work.

## Install / build

The CLI is the `echophrase-cli` member of the repo's cargo workspace. Like every
workspace member, it builds into `src-tauri/target` (see `.cargo/config.toml`), next
to the desktop app, so its binary is named `echophrase-cli` to never overwrite the
app's `echophrase` binary.

```bash
# from the repo root
cargo install --path cli          # installs echophrase-cli into ~/.cargo/bin (on PATH)
# or just build it:
cargo build --release -p echophrase-cli
# binary: src-tauri/target/release/echophrase-cli
```

### System audio dependencies (the default `mic` feature)

| Platform | Requirement |
|----------|-------------|
| Linux    | ALSA dev headers - `sudo apt install libasound2-dev` (or distro equivalent) |
| macOS    | None beyond the OS (CoreAudio) |
| Windows  | None beyond the OS (WASAPI) |

#### Headless / CI builds

If you only need to compile the orchestration (no microphone, no system audio
libs), build without the default feature. The recorder then writes silence so
the full record/stop/session flow still runs:

```bash
cargo build --no-default-features
```

## Usage

```text
echophrase-cli <COMMAND>

Commands:
  record      Start recording the microphone
  stop        Stop the active recording and finalize the WAV file
  status      Show whether a recording is currently in progress
  transcode   Convert an existing WAV file to 16 kHz mono (downmix + resample + normalize)
  transcribe  Transcribe a WAV file using the running desktop app's loaded model
  mcp         Run as a minimal MCP server so AI agents can drive voice capture
  help        Print this message or the help of the given subcommand(s)
```

### Transcode an existing file

```bash
# Downmix to mono, resample to 16 kHz, peak-normalize -> <input>.16k.wav
echophrase-cli transcode meeting.wav
# ■ Wrote meeting.16k.wav (63s, 1.9 MiB, 16000 Hz mono) (resampled from 48000 Hz)

echophrase-cli transcode in.wav -o out.wav   # explicit output
echophrase-cli transcode in.wav --no-normalize  # preserve original levels
```

Every command has detailed help:

```bash
echophrase-cli --help
echophrase-cli record --help
echophrase-cli stop --help
```

### Record and stop

```bash
# Start recording in the background; returns immediately.
echophrase-cli record
# ● Recording started (pid 12345).
#   Output: /home/you/.local/share/echophrase-cli/recordings/echophrase-1750000000.wav
#   Stop with: echophrase-cli stop

# Check on it any time.
echophrase-cli status

# Finish and finalize the WAV.
echophrase-cli stop
# ■ Saved .../echophrase-1750000000.wav (7s, 224.1 KiB)
```

### Record to a specific file or device

```bash
echophrase-cli record --output ./meeting.wav --device "USB Microphone"
```

### Foreground mode

Record in the current terminal and stop with Ctrl+C (or `echophrase-cli stop` from
another terminal):

```bash
echophrase-cli record --foreground
# ● Recording... press Ctrl+C to stop.
```

### Recovering a stuck session

If a previous recording did not shut down cleanly:

```bash
echophrase-cli record --force   # clears the stale session and starts fresh
```

### Transcribe an existing file (requires the desktop app running)

`transcribe` sends a WAV file to the already-running Echophrase desktop app
over a localhost-only IPC connection and prints back the transcript. The
desktop app owns the loaded transcription model, so it must be running:

```bash
echophrase-cli transcribe meeting.16k.wav
# test one two
```

If the app isn't running, the command exits with:

```text
error: The Echophrase desktop app must be running to transcribe. Start the app and try again.
```

### MCP server

Run `echophrase-cli mcp` to expose `record`, `stop`, `status`, `transcode`, and
`transcribe` as MCP tools over stdio. The hidden `worker` subcommand is never
exposed. Every tool shares the same session and audio logic as the CLI
commands above, so behavior never drifts between the two front ends. Like the
CLI command, the `transcribe` tool requires the desktop app to be running and
returns the same friendly error if it isn't.

Add it to a client's `.mcp.json` (Claude Code, for example):

```json
{
  "mcpServers": {
    "echophrase": {
      "command": "echophrase-cli",
      "args": ["mcp"]
    }
  }
}
```

Build without the `mic` feature (`cargo build --no-default-features`) to run
the server headless - useful for CI, where the recorder writes silence
instead of capturing real audio.

## How it works

`record`, the background `worker`, and `stop` coordinate purely through files -
no OS signals - so behaviour is identical on Linux, macOS, and Windows.

```text
$ECHOPHRASE_HOME (or platform data dir)/echophrase-cli/
├── run/
│   ├── session.json   active recording: { pid, output_path, started_at, foreground }
│   ├── stop.flag       created by `stop`, polled by the recorder
│   └── worker.log      stdout/stderr of the detached background worker
└── recordings/         default output location for WAV files
```

1. `record` picks an output path, spawns a detached `worker` process, and writes
   `session.json`.
2. The `worker` captures audio into the WAV until `stop.flag` appears.
3. `stop` writes `stop.flag`, waits for the worker to remove `session.json`, then
   reports the saved file.

See [`docs/cli.md`](docs/cli.md) for the full command reference.

[`cpal`]: https://crates.io/crates/cpal
