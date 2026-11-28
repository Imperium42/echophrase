# Handoff: macOS session (overlay NSPanel + Esc cancel check)

Written 2026-11-28 on Windows, for the next session on the Apple Silicon Mac.
Start from `main` at or after `76431ac`. Delete this file when the work below is done.

## State on `main`

- **Recording overlay pill** (`src-tauri/src/overlay/`, PR #42): a second window
  `recording_overlay` (route `src/routes/overlay`), driven only by `tray_status::set`.
  Verified on Windows only.
  - Windows has real native code: `platform_show` / `platform_hide` (`#[cfg(windows)]`,
    `SetWindowPos` topmost + no-activate).
  - macOS falls into the `#[cfg(not(windows))]` stubs: a plain Tauri window built with
    `transparent`, `always_on_top`, `visible_on_all_workspaces`, `focusable(false)`.
    Never run on a Mac. `macOSPrivateApi: true` is already set in `tauri.conf.json5`
    (needed for transparency; it means no Mac App Store).
- **Global Esc cancel** (`src-tauri/src/cancel_hotkey.rs`, PR #43): Esc is registered as a
  global shortcut only during a push-to-talk Recording (not Transcribing, not open mic),
  and a press emits `overlay-cancel` to the main window. Setting `hotkey_cancel_enabled`
  (default on, Settings > Hotkeys). Verified on Windows (Esc from another app cancels).
- **Line endings**: `.gitattributes` pins LF (PR #44). A fresh Mac clone needs nothing.

## Task 1: native overlay window (main work)

Goal: the pill floats over full-screen apps and across Spaces and can NEVER take key focus
(pasting at the cursor depends on the user's app keeping focus).

- Convert the overlay window to a non-activating `NSPanel` via the `tauri-nspanel` crate
  (this is what Handy does: github.com/cjpais/Handy, MIT; credit it in code like
  `overlay/mod.rs` already does).
  - Check the crate's current version and Tauri 2 support before adding it; it has been
    distributed as a git dependency, so confirm how it is published now.
  - Check its license against `src-tauri/about.toml` `accepted` (CLAUDE.md rule).
  - Panel wants: non-activating style mask, floating/status window level, can join all
    Spaces + full-screen auxiliary collection behavior, ignores mouse except the X.
- Replace the `#[cfg(not(windows))]` `platform_show` / `platform_hide` with a
  `#[cfg(target_os = "macos")]` pair; keep the Linux path as today.
- Optional, separate commit: native HUD vibrancy (frosted glass) behind the pill. Tauri can
  apply window effects directly; do not add a crate just for this.
- Update the macOS TODO in the `overlay/mod.rs` header comment and the overlay bullet in
  root `TODO.md` when done.

## Task 2: verify on the Mac (no code expected)

Use the hands-free E2E recipe in `CLAUDE.md` (F13 + `say` + `pbpaste`) where it fits.

- Overlay shows on record, hides on stop/cancel, sits on the monitor under the cursor,
  and the frontmost app keeps focus (paste still lands there).
- Overlay over a full-screen app and on another Space.
- Esc during a hotkey recording, from another app: pill disappears, nothing pasted.
- Esc while idle and during transcription: reaches the frontmost app normally.
- Esc with Settings > Hotkeys > "Esc cancels recording" off: ignored by Echophrase.
- Open mic: Esc is left alone.
- macOS may require Input Monitoring / Accessibility for global shortcuts; note anything
  new in `MAC.md`.

## Rules that bite here

- Branch off `main`, PR, CI green, squash-merge. macOS clippy/tests only run on pushes to
  `main` (runners queue ~10 min), so a Mac-only break shows up after merge; run
  `cargo clippy` and `cargo test -p echophrase --lib` locally on the Mac first.
- Never run test suites in the foreground (see `CLAUDE.md`).
- Any new user-facing string: all three locales. User-visible change: fold it into the
  existing unreleased "Recording Overlay" bullet in `CHANGELOG.md` (no separate
  Fixed/Improved bullets for unreleased features).
- No em or en dashes anywhere.
