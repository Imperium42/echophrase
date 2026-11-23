<div align="center">

<a href="https://echophrase.com"><img src="static/images/echophrase-icon.svg" alt="Echophrase logo" width="96" height="96"></a>

# Echophrase

### Just speak. It's already typed.

Free, open-source, GPU-accelerated speech-to-text for your desktop. Nothing leaves your machine.<br>
Press a hotkey, speak, and the text lands wherever your cursor is.

[![License: MPL 2.0](https://img.shields.io/badge/license-MPL--2.0-brightgreen?style=for-the-badge)](LICENSE)
[![Latest release](https://img.shields.io/github/v/release/imperium42/echophrase-releases?include_prereleases&label=release&color=22d3ee&style=for-the-badge)](https://github.com/imperium42/echophrase-releases/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/imperium42/echophrase-releases/total?color=a855f7&style=for-the-badge)](https://github.com/imperium42/echophrase-releases/releases)
[![CI](https://img.shields.io/github/actions/workflow/status/Imperium42/echophrase/ci.yml?branch=main&label=CI&style=for-the-badge)](https://github.com/Imperium42/echophrase/actions/workflows/ci.yml)

[![Windows](https://img.shields.io/badge/Windows-11-0078D4?logo=windows&logoColor=white)](#install)
[![macOS](https://img.shields.io/badge/macOS-13.3%2B%20Apple%20Silicon-000000?logo=apple&logoColor=white)](#install)
[![Linux](https://img.shields.io/badge/Linux-build%20from%20source-FCC624?logo=linux&logoColor=black)](#build-from-source)
[![Built with Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://v2.tauri.app)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=black)](https://www.rust-lang.org)
[![Svelte 5](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2?logo=discord&logoColor=white)](https://discord.com/invite/Er6NUcx)

[**Download**](https://echophrase.com/download) &nbsp;&middot;&nbsp;
[**Docs**](https://docs.echophrase.com) &nbsp;&middot;&nbsp;
[**Quickstart**](https://docs.echophrase.com/quickstart) &nbsp;&middot;&nbsp;
[**Report a bug**](https://github.com/Imperium42/echophrase/issues/new/choose) &nbsp;&middot;&nbsp;
[**Sponsor**](https://github.com/sponsors/imperium42)

<a href="https://github.com/sponsors/imperium42"><img src="https://img.shields.io/badge/Buy%20me%20a%20coffee-GitHub%20Sponsors-ea4aaa?style=for-the-badge&logo=githubsponsors&logoColor=white" alt="Buy me a coffee via GitHub Sponsors"></a>

</div>

---

## Why Echophrase

| | |
|---|---|
| **It just works** | Install, press the hotkey, talk. No CUDA, no Python, no driver or toolkit installs, no model wrangling: the GPU runtime ships inside the app and models download themselves. No compatibility hell, unlike most local speech-to-text setups. |
| **Private by design** | Audio is captured in memory and transcribed on your device. No account, no cloud, no upload. No telemetry and no analytics: the only network traffic is model downloads (Hugging Face) and update checks (CrabNebula). |
| **Fast** | GPU inference via ONNX Runtime (DirectML on Windows, CoreML on Apple Silicon). Transcripts in well under real time. |
| **Light** | Tauri + Rust, not a bundled browser. A fraction of an Electron app's size and memory. See [BENCHMARKS.md](BENCHMARKS.md). |
| **Free, for real** | Every feature, no tiers, no trials, no limits. [MPL-2.0](LICENSE) licensed. |

Speaking runs at ~150 words per minute; typing at ~40. Echophrase is built for long prompts to AI
assistants, docs, messages and anything else you would rather say than type.

## Features

- **Global hotkeys** - push to talk (tap or hold) or toggle, from any app
- **Paste at cursor** or copy to clipboard, with optional clipboard restore after pasting
- **Open mic** - hands-free dictation with voice activity detection (Silero VAD)
- **Local speech models** - auto-downloaded on first use, integrity-checked, [25+ languages](#models)
- **Grammar cleanup** - optional on-device T5 pass after transcription
- **Bring your own model** - load any compatible ONNX speech model ([docs](https://docs.echophrase.com/features/custom-models))
- **CLI and MCP server** - script it, or let AI agents record and transcribe ([CLI](https://docs.echophrase.com/cli), [MCP](https://docs.echophrase.com/mcp))
- **JSON output with timestamps** and compute-type selection
- **Localized** in English and Chinese (Simplified and Traditional) for now; more languages welcome

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

Or grab the installer from [echophrase.com/download](https://echophrase.com/download). The app updates itself.

| Platform | Status | GPU |
|---|---|---|
| **Windows 11** | Official, signed builds. Windows 10 should work but is not officially supported. | DirectML (any DirectX 12 GPU), bundled |
| **macOS 13.3+ (Apple Silicon)** | Official, signed and notarized builds. Intel Macs are not supported ([MAC.md](MAC.md)). | CoreML / Metal |
| **Linux** | Community-supported: [build from source](#build-from-source). No official binaries; PRs welcome. | CPU only today |

<details>
<summary><b>Why not the Mac App Store?</b></summary>

App Store apps must run in Apple's sandbox, and sandboxed apps can't send the synthetic keystroke
(Cmd+V) that pastes your transcript into the focused app. Auto-paste is the core feature, so
Echophrase ships outside the store, signed with a Developer ID and notarized by Apple.
</details>

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

## Privacy

Your voice and your transcripts never leave your device. No telemetry and no analytics. The only
network traffic is model downloads (Hugging Face) and update checks (CrabNebula). Details in
[TELEMETRY.md](TELEMETRY.md).

## Build from source

**Prerequisites:** [Rust](https://rustup.rs) (stable, 1.88+), [Bun](https://bun.sh), and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.
[Node.js](https://nodejs.org) is also needed to run the frontend tests (vitest runs on Node).

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

<details>
<summary><b>Production build, checks and dev port</b></summary>

**Production build:** `bun run tauri:build` (output in `src-tauri/target/release/bundle/`).

**Checks** (the same ones CI runs on every PR):

```bash
bun run check && bun run lint && bun run test:run
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
```

**Dev port:** the Vite dev server uses `DEV_PORT` (default `8420`, HMR on `8421`). If that port is
taken, set `DEV_PORT` in your gitignored `env.yaml` instead of editing tracked files.
</details>

## Troubleshooting

<details>
<summary><b>Common fixes</b></summary>

- **System tray icon missing (Ubuntu/GNOME):** `sudo apt install gnome-shell-extension-appindicator && gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com`
- **`gobject-2.0` / `pango` not found:** install the [Linux system packages](#build-from-source).
- **No microphone on WSL:** install `libasound2-plugins`; see [docs/WSL.md](docs/WSL.md).
- **Debug panel (dev builds only):** click "Debug" (top-left) for logs and quick actions; F4 restores it after "Hide 5m".

More in the [troubleshooting docs](https://docs.echophrase.com/troubleshooting/common-issues).
</details>

## Contributing

Bugs and ideas go in [issues](https://github.com/Imperium42/echophrase/issues). Read
[CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR, and [SECURITY.md](SECURITY.md) to report a
vulnerability privately. Architecture notes for contributors (and AI coding agents) are in
[CLAUDE.md](CLAUDE.md); workarounds for upstream bugs are in [KNOWN_ISSUES.md](KNOWN_ISSUES.md).

Questions, ideas or just want to chat? Join us on [Discord](https://discord.com/invite/Er6NUcx).

## Support the project

Echophrase is free, with no ads, no upsells, no telemetry and no paid tier.

If you found this app useful, consider [contracting me](https://imperium42.com) - or buy me a coffee:

<a href="https://github.com/sponsors/imperium42"><img src="https://img.shields.io/badge/Sponsor%20on%20GitHub-ea4aaa?style=for-the-badge&logo=githubsponsors&logoColor=white" alt="Sponsor on GitHub"></a>

## License

Echophrase is open source under the [Mozilla Public License 2.0](LICENSE) (MPL-2.0). Third-party
code notices are in [ATTRIBUTION.md](ATTRIBUTION.md); model licenses are listed under [Models](#models).

The license covers the code, not the name: "Echophrase" and the logo are trademarks of
Imperium42 LLC, so forks must ship under a different name. See [TRADEMARKS.md](TRADEMARKS.md).

<div align="center">
<sub>Copyright &copy; 2025-2026 Imperium42 LLC &middot; Made with Rust, Tauri and too much coffee</sub>
</div>
