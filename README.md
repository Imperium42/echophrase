# Echophrase

<div align="center">

[![CI](https://github.com/Imperium42/echophrase/actions/workflows/ci.yml/badge.svg)](https://github.com/Imperium42/echophrase/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/imperium42/echophrase-releases?include_prereleases&label=release&color=22d3ee)](https://github.com/imperium42/echophrase-releases/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/imperium42/echophrase-releases/total?color=a855f7)](https://github.com/imperium42/echophrase-releases/releases)
[![Homebrew](https://img.shields.io/badge/homebrew-imperium42%2Ftap-f9d094?logo=homebrew&logoColor=white)](https://github.com/imperium42/homebrew-tap)
[![winget](https://img.shields.io/badge/winget-Imperium42.Echophrase-0078d4?logo=windows&logoColor=white)](https://winstall.app/apps/Imperium42.Echophrase)
[![Scoop](https://img.shields.io/badge/scoop-echophrase-4c8eda?logo=windows&logoColor=white)](https://github.com/imperium42/scoop-bucket)
[![Website](https://img.shields.io/badge/echophrase.com-visit-22d3ee)](https://echophrase.com)
[![Discord](https://img.shields.io/badge/discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/Er6NUcx)
[![Sponsor](https://img.shields.io/badge/sponsor-%E2%9D%A4-ea4aaa?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/imperium42)

</div>

Privacy-first, GPU-accelerated speech-to-text for your desktop. Press a hotkey, talk, and the
text lands where your cursor is. Everything runs locally: no account, no cloud, free and open source.

## Install

```bash
# macOS (Apple Silicon)
brew install --cask imperium42/tap/echophrase

# Windows (winget)
winget install Imperium42.Echophrase

# Windows (Scoop)
scoop bucket add echophrase https://github.com/imperium42/scoop-bucket
scoop install echophrase
```

Or download the installer from [echophrase.com](https://echophrase.com). The app updates itself.

| Platform | Status |
|----------|--------|
| **Windows 10/11** | Official builds. GPU via DirectML (any DirectX 12 GPU). |
| **macOS 13.3+ (Apple Silicon)** | Official builds, signed and notarized. GPU via CoreML/Metal. Intel Macs are not supported ([MAC.md](MAC.md)). |
| **Linux** | **Community-supported, build from source.** CPU-only today. No official binaries; PRs welcome. |

**Why no Mac App Store?** App Store apps must run in Apple's sandbox, and sandboxed apps can't
send the synthetic keystroke (Cmd+V) that pastes your transcript into the focused app. Auto-paste
is the core feature, so Echophrase ships outside the store, signed with a Developer ID and
notarized by Apple.

## Why Echophrase?

- **Speaking is faster than typing.** ~150 WPM spoken vs ~40 WPM typed. Great for long prompts to AI assistants, docs and messages.
- **Private by design.** Audio is captured in memory and transcribed on your machine. It is never uploaded.
- **Fast.** GPU-accelerated inference with ONNX Runtime; transcripts in well under real time.
- **Light.** Built with Tauri and Rust, not a bundled browser.

## Features

- Global push-to-talk / toggle hotkeys, paste-at-cursor or clipboard output
- Open-mic mode with voice activity detection (Silero VAD)
- Multiple local speech models, auto-downloaded on first use with integrity checks
- Optional grammar cleanup (T5) after transcription
- Bring your own ONNX model
- English, Simplified Chinese and Traditional Chinese UI

## Models

Models download from Hugging Face on first use into `~/.cache/huggingface/hub/`. They are not part
of this repository, and each keeps its own license:

| Model | Use | Size | Languages | License |
|-------|-----|------|-----------|---------|
| [NVIDIA Parakeet-TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) ([ONNX](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx)) | Speech-to-text (default) | 2.5 GB | 25 European | CC-BY-4.0 |
| [Qwen3-ASR 0.6B](https://huggingface.co/Qwen/Qwen3-ASR-0.6B) ([ONNX](https://huggingface.co/andrewleech/qwen3-asr-0.6b-onnx)) | Speech-to-text | 2.0 GB | 52, incl. Chinese | Apache-2.0 |
| [NVIDIA Parakeet Realtime EOU 120M](https://huggingface.co/nvidia/parakeet_realtime_eou_120m-v1) ([ONNX](https://huggingface.co/altunenes/parakeet-rs)) | Speech-to-text ("Parakeet Lite") | 0.5 GB | English | NVIDIA Open Model License |
| [T5-efficient-mini grammar correction](https://huggingface.co/visheratin/t5-efficient-mini-grammar-correction) | Grammar cleanup | 60 MB | English | MIT |
| [Silero VAD v5](https://github.com/snakers4/silero-vad) | Voice activity detection (bundled) | 2 MB | any | MIT |

## Building from source

Prerequisites: [Rust](https://rustup.rs) (stable, 1.88+), [Bun](https://bun.sh), and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
git clone https://github.com/Imperium42/echophrase.git
cd echophrase
bun install
bun run tauri:dev        # auto-detects your platform
```

The first `tauri:dev` (or `tauri:build`) downloads the pinned, hash-verified ONNX Runtime the
releases ship. On Windows it lands in `src-tauri/resources/ort/directml/` (no config needed); on
Linux/WSL/macOS in a gitignored `.ort/` folder, with `ORT_DYLIB_PATH` written into your `env.yaml`
(created from `env.yaml.template`).

<details>
<summary><b>Linux system packages</b></summary>

```bash
# Ubuntu/Debian
sudo apt install build-essential curl wget file libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev libasound2-dev

# Fedora
sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file \
  libappindicator-gtk3-devel librsvg2-devel libxdo-devel alsa-lib-devel

# Arch
sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl \
  appmenu-gtk-module libappindicator-gtk3 librsvg xdotool alsa-lib
```

On WSL, see [docs/WSL.md](docs/WSL.md). Linux runs on CPU; see
[docs/gpu-and-cuda.md](docs/gpu-and-cuda.md) for why there is no CUDA path.
</details>

<details>
<summary><b>Windows</b></summary>

Install Visual Studio 2022 (or Build Tools) with the **Desktop development with C++** workload.
Then `bun run tauri:dev:windows`, or `scripts/win/dev.ps1`. The DirectML runtime is bundled at
build time; nothing else to install.
</details>

<details>
<summary><b>macOS</b></summary>

Xcode Command Line Tools (`xcode-select --install`), then `bun run tauri:dev:mac`.
</details>

**Production build:** `bun run tauri:build` (output in `src-tauri/target/release/bundle/`).

**Checks** (the same ones CI runs on every PR):

```bash
bun run check && bun run lint && bun run test:run
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
```

**Dev port:** the Vite dev server uses `DEV_PORT` (default `8420`, HMR on `8421`). If that port is
taken, set `DEV_PORT` in your gitignored `env.yaml` instead of editing tracked files.

Architecture notes for contributors (and AI coding agents) are in [CLAUDE.md](CLAUDE.md).
Workarounds for upstream bugs are in [KNOWN_ISSUES.md](KNOWN_ISSUES.md).

## Troubleshooting

**System tray icon missing (Ubuntu/GNOME):** install and enable AppIndicator support:
`sudo apt install gnome-shell-extension-appindicator && gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com`

**`gobject-2.0` / `pango` not found:** install the Linux system packages above.

**No microphone on WSL:** install `libasound2-plugins`; see [docs/WSL.md](docs/WSL.md).

**Debug panel (dev builds only):** click "Debug" (top-left) for logs and quick actions; F4 restores it after "Hide 5m".

## Privacy

Your voice never leaves your device. Official builds send **anonymous** usage stats (OS, app
version, GPU class, crashes; no user ID, no audio, no text), and you can turn them off in
Settings > Data. Builds from source send nothing. Details: [TELEMETRY.md](TELEMETRY.md).

## Support the project

Echophrase is free and community-maintained. If it saves you time, consider
[sponsoring on GitHub](https://github.com/sponsors/imperium42). Bugs and ideas go in
[issues](https://github.com/Imperium42/echophrase/issues); see [CONTRIBUTING.md](CONTRIBUTING.md)
before opening a PR.

## License

Echophrase is open source under the [Mozilla Public License 2.0](LICENSE). Third-party code
notices are in [ATTRIBUTION.md](ATTRIBUTION.md); model licenses are listed under [Models](#models).

The license covers the code, not the name: "Echophrase" and the logo are trademarks of
Imperium42 LLC, so forks must ship under a different name. See [TRADEMARKS.md](TRADEMARKS.md).

Copyright © 2025-2026 Imperium42 LLC.
