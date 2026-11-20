# Echophrase CLI - command reference

`echophrase` is a cross-platform microphone recorder for the terminal. It
records 16-bit mono WAV files and also runs as a minimal MCP server so AI
agents can drive voice capture.

## Synopsis

```text
echophrase <COMMAND> [OPTIONS]
```

## Global behaviour

- All state lives under a single base directory. Resolution order:
  1. `$ECHOPHRASE_HOME`
  2. platform data dir + `echophrase-cli`
     - Linux: `$XDG_DATA_HOME` or `~/.local/share`
     - macOS: `~/Library/Application Support`
     - Windows: `%LOCALAPPDATA%`
  3. system temp directory (last resort)
- Exit code `0` on success, `1` on error (message printed to stderr).

## Commands

### `echophrase record`

Start recording the microphone.

| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Destination WAV path. Default: a timestamped file in `<base>/recordings/`. |
| `-d, --device <NAME>` | Input device name. Default: the system default input. |
| `-f, --foreground` | Record in the current process until Ctrl+C, instead of detaching a background worker. |
| `--force` | Clear a stale session and start a new recording. |

Default (background) mode spawns a detached worker and returns immediately.
Foreground mode blocks and can be stopped with Ctrl+C or `echophrase stop`.

Errors if a recording is already in progress (unless `--force`).

### `echophrase stop`

Stop the active recording and finalize the WAV file.

| Option | Description |
|--------|-------------|
| `-t, --timeout <SECS>` | Seconds to wait for the background worker to finalize. Default: `10`. |

For a foreground recording, `stop` delivers the stop request and returns; the
foreground process prints its own summary.

Errors if there is no active recording.

### `echophrase status`

Print whether a recording is in progress, and if so its pid, mode
(foreground/background), elapsed time, and output path.

### `echophrase transcribe <FILE>`

Transcribe a WAV file using the running desktop app's already-loaded
transcription model.

This talks to the Echophrase desktop app over a localhost-only IPC
connection (the app owns the model; this CLI does not load its own copy).
**The desktop app must be running.** If it isn't, or the CLI can't reach it,
the command exits with:

```text
error: The Echophrase desktop app must be running to transcribe. Start the app and try again.
```

Prints the transcript to stdout on success.

### `echophrase mcp`

Run as a minimal MCP (Model Context Protocol) server over stdio.

Exposes five tools, backed by the same session/audio logic as the CLI
commands above:

| Tool | Input | Output |
|------|-------|--------|
| `record` | `device?`, `force?` | output WAV path, worker pid |
| `stop` | `timeout_secs?` | final WAV path, duration, size |
| `status` | - | recording? + session info |
| `transcode` | `input`, `output?`, `normalize?` | canonical 16 kHz WAV path |
| `transcribe` | `input` | transcribed text (requires the desktop app running) |

The hidden `worker` subcommand is never exposed as a tool. Runs until stdin
closes. See [`README.md`](../README.md#mcp-server) for a Claude Code
`.mcp.json` example.

### `echophrase help [COMMAND]`

Print help for the CLI or a specific subcommand. Equivalent to `--help`.

## Examples

```bash
# Background record, then stop.
echophrase record
echophrase status
echophrase stop

# Named output and explicit device.
echophrase record -o ./take1.wav -d "USB Microphone"

# Foreground (Ctrl+C to stop).
echophrase record --foreground

# Recover after an unclean shutdown.
echophrase record --force

# Use a custom state directory.
ECHOPHRASE_HOME=/tmp/ep echophrase record
```

## Notes

- Output is always **16 kHz, 16-bit, mono** - the format Echophrase's
  transcription pipeline expects. Most devices capture at 44.1/48 kHz, so audio
  is captured at the device rate and resampled to 16 kHz (linear interpolation,
  the same method the transcription backends use). The captured samples are
  buffered in memory for the duration of the recording; a streaming resampler
  would lift that for very long captures.
- The hidden `worker` subcommand is an implementation detail of background mode
  and should not be invoked directly.
