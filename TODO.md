# TODO

Planned work that isn't scheduled yet. Shipped items move to `CHANGELOG.md`.

## Features

- **Official bilingual dictation (two languages at once, eg English + Chinese).**
  This already works unofficially on models that support code-switching
  (Qwen3-ASR covers both), but it has only had minimal testing. To make it official:
  - Test mixed English/Chinese (and other pairs) per model and document which models support it.
  - Add a bilingual test fixture to `tests/audio/`.
  - Make the post-processing steps language-aware (filler-word removal, number
    normalization and spoken "question mark" / "exclamation mark" are English-only today).
  - Say which models support it in the UI and the docs.

- **Always-on-top recording overlay.** A small indicator that floats above all apps
  while recording, so you can see it's listening without switching windows.
  - Research the permissions first, per platform: macOS (floating panel across Spaces and
    over full-screen apps; does it need Accessibility or Screen Recording?), Windows
    (topmost window over full-screen/exclusive apps and elevated windows), Linux/Wayland
    (compositors may refuse always-on-top).
  - Must not steal focus, or paste-at-cursor lands in the overlay instead of the target app.
  - Click-through so it never blocks what's under it.

- **Drop-a-file overlay (file to text).** A small floating drop zone plus a file picker: drop
  an audio file, get the transcript (copy / paste / save, same output modes as recording).
  - Reuse the existing file path (`transcribe_audio` / `AudioSource::File`, the same one
    `echophrase-cli transcribe` uses).
  - Needs a decoder first: only WAV is read today. Add MP3/M4A/AAC/OGG/FLAC (eg `symphonia`;
    check its license against `src-tauri/about.toml`) and resample through `echo_audio`.
  - Long files: Parakeet is chunked over 40s, Qwen3-ASR and Candle are not yet, so chunk or
    cap them. Show progress.
  - Shares the overlay research above (focus, click-through except the drop zone, Wayland).
  - Elevated windows: dragging from normal Explorer into an elevated app is blocked (see the
    admin item below).

- **Work in admin / secure-input windows.**
  - Windows: tell the user instead of failing silently. Before pasting, check if the focused
    window's process is elevated (no permission needed); if so, show "Copied - can't paste into
    admin windows". macOS equivalent: `IsSecureEventInputEnabled()` (password fields,
    Terminal's Secure Keyboard Entry).
  - Windows: optional "Run as administrator" setting, off by default (PowerToys-style).
    Relaunch elevated on toggle (one UAC prompt); start-with-Windows via a Task Scheduler task with
    highest privileges so login does not prompt. Cost: drag-and-drop from normal Explorer into an
    elevated window is blocked, so it would break the planned drop-a-file overlay. Say so in the hint.
  - Not viable: a `uiAccess` manifest (needs a signed exe under Program Files; we install
    per-user, and a uiAccess exe outside a secure location refuses to start).
