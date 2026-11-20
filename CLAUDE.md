# CLAUDE.md

Guidance for contributors and AI coding agents (Claude Code, etc.) working in this repository.
See also [CONTRIBUTING.md](CONTRIBUTING.md). Maintainer-only notes can go in a gitignored `CLAUDE.local.md`.

> **Current date context:** It's 2026. When searching for packages, docs, or solutions, search 2025/2026 (never earlier).

## Essential Commands

```bash
# Development (auto-detects platform)
bun run tauri:dev                  # Recommended default
bun run tauri:dev:wsl              # Force WSL with GPU passthrough
bun run tauri:dev:windows          # Force Windows native

# Building
bun run tauri:build                # Production build (outputs to src-tauri/target/release/bundle/)

# Testing
bun test                           # Frontend tests (Vitest, watch mode)
bun run test:run                   # Frontend tests (single run)
bun run test:rust                  # All Rust backend tests
bun run test:transcription         # Transcription test (WSL/Linux)
bun run test:transcription:win     # Transcription test (Windows DirectML - needs native Windows path)
bun run test:vad                   # VAD tests

# Code Quality
bun run check                      # Svelte type checking
bun run lint && bun run lint:fix   # ESLint
bun run format                     # Prettier
cd src-tauri && cargo clippy       # Rust linting
cd src-tauri && cargo fmt          # Rust formatting

# Package Management (Bun only - never npm/yarn)
bun install                        # Install dependencies
bun add <package>                  # Add runtime dependency
bun add -d <package>               # Add dev dependency
cd src-tauri && cargo add <crate>  # Add Rust crate
```

## Architecture Overview

### Tech Stack
- **Frontend**: Svelte 5 + SvelteKit + TypeScript
- **Backend**: Tauri 2.0 (Rust 2024 edition)
- **ML Frameworks**: Trait-based backends - Candle (safetensors), ONNX Runtime (parakeet-rs for NVIDIA TDT)
- **Audio**: `crates/echo-audio` (shared audio core: cpal capture, resampling, WAV I/O, AudioSource) + Voice Activity Detection (self-implemented Silero VAD V5). Live capture is fully in-memory on all platforms - no temp WAVs.
- **GPU**: DirectML (Windows, plus an opt-in CUDA plugin), CoreML (macOS); Linux/WSL is CPU-only. ORT is loaded dynamically via ort `load-dynamic`
- **No server, no account**: since the v1.0.0 open-source move, the app is fully free and local. There is no login, tier, trial, payment, heartbeat, MOTD or server cloud sync. The only network calls are model downloads, the CrabNebula updater, the GitHub release archive and (official builds only) Aptabase telemetry.
- **Icons**: import `<Icon>` from `$lib/icons` only — never directly from `@lucide/svelte` or `simple-icons`. Add new icons to `src/lib/icons/registry.ts` (Lucide v1 for functional, Simple Icons for brand logos).
- **Styling**: Tailwind CSS 4.x

### Frontend-Backend Communication

Tauri commands bridge JavaScript and Rust. All commands are registered in `src-tauri/src/lib.rs:invoke_handler`. Frontend invokes via:
```typescript
import { invoke } from '@tauri-apps/api/core';
const result = await invoke('command_name', { param: value });
```

Event streaming for real-time updates (download progress, VAD status) uses Tauri's event system.

### Key Frontend Directories

```
src/lib/
├── components/          # Svelte components
│   ├── settings/       # Settings modal with sections pattern
│   └── ui/             # Reusable primitives (select, etc.)
├── stores/             # Svelte stores (model, settings, theme, ...)
├── services/           # API/backend communication layer
├── config/             # Environment config (environment.ts is source of truth)
├── composables/        # Reusable reactive logic
├── hooks/              # Lifecycle hooks
├── types/              # TypeScript types
└── utils/              # Helper functions
```

### Key Backend Modules (`src-tauri/src/`)

```
commands/               # Tauri command handlers
├── transcription.rs    # Main transcription logic (~65KB, core functionality)
├── recording.rs        # Audio recording control
├── model_detection.rs  # VRAM analysis & model auto-selection
├── vad.rs              # Voice Activity Detection test commands
└── output.rs           # Paste simulation, file save

transcription/          # ML backends (trait-based, swappable)
├── backend.rs          # TranscriptionBackend trait - interface for all backends
├── candle_whisper.rs   # Candle backend (Whisper safetensors, CUDA)
├── onnx_backend.rs     # ONNX backend (generic ONNX models)
└── parakeet.rs         # parakeet-rs backend (NVIDIA TDT with RNN-T decoding)

vad.rs                  # Voice Activity Detection
                        # VadBackend trait + SileroVad implementation (ort)
                        # Self-implemented to avoid dependency version conflicts

model_detection/        # Auto model selection
├── vram_analyzer.rs    # GPU VRAM detection
└── model_selector.rs   # Model recommendation logic
```

### Speech-to-Text Pipeline

1. **Recording**: Rust `AudioRecorder` (echo-audio/cpal) captures 16 kHz mono f32 chunks in memory on every platform
2. **VAD** (optional): Silero VAD V5 detects speech segments (self-implemented with ort)
3. **Backend selection**: TranscriptionBackend trait dispatches to appropriate backend
4. **Preprocessing**: Audio → 16kHz mono → mel spectrogram (80 bins Whisper, 128 bins Parakeet)
5. **Inference**: GPU-accelerated (DirectML on Windows, CoreML/Metal on macOS), CPU on Linux/WSL
6. **Output**: Transcription → clipboard or paste-at-cursor

### Model Support

| Backend | Models | Format | GPU Provider | VRAM |
|---------|--------|--------|--------------|------|
| Candle | Distil-Whisper Large-v3 (recommended), Whisper variants | safetensors | Metal/CPU (CUDA cfg dormant) | 2-6GB |
| parakeet-rs | NVIDIA Parakeet-TDT 0.6B v3 | ONNX (TDT decode) | DirectML/CoreML/CPU | 2GB |
| ONNX (generic) | Any ONNX model | ONNX | DirectML/CoreML/CPU | Varies |

Models auto-download to `~/.cache/huggingface/hub/` on first use with progress streaming.

**Parakeet TDT Gotcha**: Always use `TimestampMode::Words` when calling `transcribe_samples()`. Without it, the TDT decoder's beam search can collapse tokens, causing entire sentences to be dropped (eg "He saves the world with his brother, Luigi" → "Luigi"). See `onnx_backend.rs:128`.

**Memory guards (Parakeet TDT)**: encoder working memory grows much faster than clip length and ORT keeps the peak, so a single 300s call pushed a real Windows install to ~4.5 GB (2026-11-14). Two guards: `transcription/chunking.rs` splits audio over 40s into ~30s chunks cut at the quietest frame near each boundary (clips up to 40s are still one call, text untouched), and `transcription/idle.rs` can unload the model after N idle minutes (OPT-IN: Settings > Optimization, `idle_unload_enabled` default false, `idle_unload_minutes` default 10, clamped 1-120, local-only; never during Recording/Transcribing; the next recording start reloads it in the background, ~7s warm). Measured numbers and the repro method: `BENCHMARKS.md`. Qwen3-ASR and Candle are NOT chunked yet.

**Model Preload Baselines** (`from_pretrained` wall time, for spotting regressions):

| Model | Provider | GPU | Shader cache | Preload time |
|-------|----------|-----|--------------|--------------|
| parakeet-tdt-0.6b-v3 | DirectML | RTX 5080 | warm | ~7.1s |

> **Warm vs cold matters more than the number.** DirectML JIT-compiles shaders on the *first* run for a given GPU architecture and caches them per machine. Warm-cache reloads (above) are fast. A cold first-run on a new GPU arch can take far longer - this is exactly why the Rust-side preload timeout is 300s (see `9ce98cc`) and why a fast dev reload does **not** prove the cold-JIT deadlock path (see `590f1b4`/`b384fc0`) is healthy. Treat ~7s as the warm regression baseline, not the worst case. See [docs/model-load-baselines.md](docs/model-load-baselines.md).

### Backend Architecture (Trait-Based)

The codebase uses **trait interfaces** for swappable implementations:

```rust
// Transcription backends: one required method + a provided entry point that
// resolves echo_audio::AudioSource { File | Memory } to 16 kHz mono samples.
// Commands call transcription::transcribe_source() - the single dispatch router.
pub trait TranscriptionBackend: Send + Sync {
    fn transcribe_samples(&self, samples: &[f32]) -> Result<String>;  // required
    fn backend_name(&self) -> &'static str;
    fn transcribe(&self, source: &AudioSource) -> Result<String>;     // provided
}

// VAD backends
pub trait VadBackend: Send {
    fn predict(&mut self, samples: &[f32]) -> f32;  // 0.0-1.0 probability
    fn reset(&mut self);
    fn name(&self) -> &'static str;
}
```

This allows:
- Swapping ONNX, Candle, or future backends (safetensors, GGUF, etc.)
- Platform-specific GPU providers (CUDA, DirectML, ROCm, CoreML, NPU)
- Auto-device EP selection via ort 2.0.0-rc.12+ `with_auto_device(MaxPerformance)`

### Environment Configuration

**`env.yaml`** (gitignored, create from `env.yaml.template`):
- Platform detection: WSL, Windows, Linux, macOS
- Hierarchical: shared → platform-specific overrides
- Loaded by `scripts/env/load.mjs` before Tauri starts (validated by `scripts/env/schema.ts`)

**`src/lib/config/environment.ts`** - Frontend URL config (website/docs links only; there is no API server).

### No tiers, no account

Everything is free for everyone. The old Pro/tier/trial/auth/usage-metering system
(`feature_gate/`, `auth/`, `commands/usage.rs`, `settings/sync/`, `featureGate` stores,
Pro modals and banners) was deleted for the v1.0.0 open-source release. Don't reintroduce
gates. The per-recording cap is a plain user setting (5-180s, Settings > Recording, default
30s), enforced by `useRecording.ts::getMaxRecordingTime()` against the hard 180s
`MAX_RECORDING_TIME_SECONDS` ceiling.

Server cloud sync was removed too. A folder-based sync (settings file in the user's own
iCloud Drive / OneDrive / Dropbox) is planned after v1.0.0. The old hash/model/resolver code
is not part of this repo.

Old installs may still have a dead auth entry in the OS keychain (`keyring` was dropped, no
cleanup code). That's harmless.

### Telemetry

See [TELEMETRY.md](TELEMETRY.md) for what data is collected.

- `src-tauri/src/telemetry/`: anonymous install/launch events, no user ID.
- **Only in official builds**, behind the `official` Cargo feature (OFF by default). Release
  CI builds with `--features official` and injects `APTABASE_APP_KEY`. Without the feature
  every telemetry call is a no-op, and the build does not need the key, so contributor/fork
  builds never report to our Aptabase instance.

## GPU Requirements

> **Verify against the code.** Docs drift; when this file and the code disagree,
> the code wins - fix the doc in the same change.

> **CUDA is NOT required to build on Linux/WSL.** macOS
> requires Apple Silicon (M1+, no Intel). Windows builds run on bundled DirectML.

### Linux/WSL (CUDA disabled)
CUDA is **off** on Linux/WSL and nothing about it is needed to build or run:

- The `cuda` cargo feature is opt-in (`default = []` in `src-tauri/Cargo.toml`)
  and dev builds run `--no-default-features`, so Candle's CUDA path is compiled out.
- `ort` uses `load-dynamic`, so ORT is loaded at runtime from `ORT_DYLIB_PATH`;
  nothing CUDA is linked or downloaded at build time.
- The CUDA link flags and env in `.cargo/config.toml` are commented out under
  the same `TODO(cuda-linux)` tag. They used to force `-lcudart` etc. into every
  Linux link, so a machine without the CUDA toolkit failed to build at all.
- The build-affecting CUDA vars are absent from `env.yaml.template` and are not
  declared in `scripts/env/schema.ts` (the schema is `.passthrough()`).
  `RUSTFLAGS` sat in cargo's fingerprint and `CC`/`CXX` pinned all native deps
  to gcc-12 for nvcc's sake; the system default gcc-13 builds fine.
- The Linux ORT preload no longer lists `libonnxruntime_providers_cuda.so` - it
  could never load (`RTLD_LOCAL` hides `providers_shared`'s `Provider_GetHost`)
  and only printed an error each launch. ORT still loads the CUDA EP itself at
  session creation via `with_auto_device`.

To re-enable: re-add the vars to your `env.yaml`, optionally re-declare them in the schema, build with
`--features cuda`. Historical host requirements, if you do: CUDA Toolkit 13.0+
with cuDNN 9.19+ (`sudo apt install cuda-toolkit-13-0 libcudnn9-cuda-13`),
driver 525+, GCC-12, 2GB+ VRAM. Blackwell (RTX 50-series) needs CUDA 13+ since
the CUDA 12 ORT prebuilt lacks sm_120 gencode.

See [docs/gpu-and-cuda.md](docs/gpu-and-cuda.md).

### Windows (DirectML default, CUDA opt-in plugin)
- **DirectML** (default): DirectX 12 compatible GPU (NVIDIA, AMD, Intel). No additional
  drivers required. Bundled with installer (~15MB).
- **CUDA** (opt-in, NVIDIA only): downloaded as a runtime plugin from Settings > GPU
  when the user has CUDA Toolkit 13.0+ and cuDNN 9.19+ installed. Plugin DLLs land in
  `%LOCALAPPDATA%/com.imperium42.echophrase/ort/cuda/` (~650MB). Activation requires app
  restart. Plugin lifecycle lives in `src-tauri/src/ort_plugin/`; runtime selection in
  `src-tauri/src/setup/ort_runtime.rs::select_ort_runtime()`.
- Automatic fallback to CPU if GPU unavailable

### macOS (Apple Silicon)
- Apple Silicon required (M1/M2/M3/M4) - Intel Macs not supported
- macOS 13.3+ (`minimumSystemVersion` in `tauri.conf.json5`; ORT 1.24 needs C++20 `std::to_chars`)
- Metal GPU acceleration via ONNX Runtime

### ONNX Runtime (load-dynamic, auto-installed for dev)

ort 2.0.0-rc.12 uses `load-dynamic` to load `onnxruntime.dll`/`.so`/`.dylib` at runtime
via the C API. This avoids MSVC STL ABI coupling from static linking (pykeio/ort#361).

- **Dev setup (Linux/WSL/macOS)**: **automatic.** `scripts/ensure-ort.mjs` runs as a
  prerequisite of `bun run tauri:dev` (and `bun run setup:ort` standalone). It
  detects the platform, downloads ORT 1.24.4 from microsoft/onnxruntime releases to
  a gitignored repo-local `.ort/<platform>/`, and writes an absolute `ORT_DYLIB_PATH`
  into the correct section of `env.yaml`. Idempotent — fast no-op when already installed.
  Skip it (e.g. in CI) by calling `bun x tauri dev` directly.
  - **macOS extra step (automatic):** on `darwin-arm64`, `ensure-ort.mjs` also copies the
    dylib to `src-tauri/macos-frameworks/libonnxruntime.dylib`. `tauri.conf.json5` declares
    that path under `bundle.macOS.frameworks`, and `tauri_build::try_build` validates it
    EXISTS at compile time - so a bare `cargo build` / `bun run tauri:dev:mac` on a fresh
    clone fails with `Library not found: ./macos-frameworks/libonnxruntime.dylib` unless the
    dylib is staged there first. Both `.ort/` and `macos-frameworks/` are gitignored.
- **Dev setup (Windows)**: use `scripts/win/dev.ps1` or the in-app CUDA plugin
  (Settings > GPU). `ensure-ort.mjs` deliberately skips Windows since DirectML is
  bundled at build time and CUDA is a user-opt-in runtime download.
- **Manual override**: setting `ORT_DYLIB_PATH` manually in `env.yaml` still works;
  the auto-script will overwrite that line on next run. If you need a custom path,
  don't run `ensure-ort.mjs` (call `bun x tauri dev` directly).
- **Windows DirectML** (bundled default - covers all GPUs):
  `Microsoft.ML.OnnxRuntime.DirectML` 1.24.2 from NuGet. `DirectML.dll` itself
  comes from Windows System32 (1.15+, shipped on Win11 22H2+).
  Download: `curl -L -o ort-dml.nupkg https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/1.24.2`
  Production path: `<install>/resources/ort/directml/onnxruntime.dll` (CI stages DLLs before `tauri build`).
- **Windows CUDA** (NVIDIA only, user-opt-in downloadable plugin):
  `onnxruntime-win-x64-gpu-1.24.2.zip` from [GitHub](https://github.com/microsoft/onnxruntime/releases/tag/v1.24.2).
  Users trigger the download from Settings > GPU > CUDA modal when both CUDA_PATH and
  CUDNN_PATH are set. `src-tauri/src/ort_plugin/downloader.rs` streams the zip, verifies
  SHA256, and extracts only `onnxruntime.dll` + `onnxruntime_providers_cuda.dll` +
  `onnxruntime_providers_shared.dll` into `%LOCALAPPDATA%/com.imperium42.echophrase/ort/cuda/`.
  CAUTION: GitHub's `-gpu-` zip has CUDA+TensorRT but NOT DirectML — you can't mix
  providers across ORT builds, which is why we ship two runtimes.
- **Linux**: `onnxruntime-linux-x64-gpu-1.24.2.tgz` (CUDA)
- **macOS**: `onnxruntime-osx-arm64-1.24.2.tgz` (CoreML)
- **Required system libraries**: NVIDIA CUDA Toolkit 13.0+ and cuDNN 9.19+ (for the
  CUDA EP - DirectML and CoreML have no system requirements beyond the OS itself).
- **EP selection**: ort's `with_auto_device(MaxPerformance)` enumerates devices at session
  creation time and picks the best one (CUDA > DirectML > CoreML > NPU > CPU). The user can
  override via Settings > GPU Acceleration radio buttons.
- **Runtime selection (Windows)**: `select_ort_runtime()` in `setup/ort_runtime.rs` runs before any ORT
  API is touched and sets `ORT_DYLIB_PATH`. Precedence: installed CUDA plugin + NVIDIA +
  CUDA_PATH/CUDNN_PATH → plugin runtime; else dev override from env.yaml; else bundled DirectML.
- **NPU auto-use**: Copilot+ PCs (Snapdragon X) and Intel Core Ultra get NPU acceleration
  for free via ort's device enumeration. No code changes needed.
- **Bundling**: The ORT DLLs must be shipped alongside the executable for production builds.

## Cargo Workspace & Audio Architecture

> **Linux/WSL build requirement:** audio capture links ALSA (cpal), so builds
> need `sudo apt install libasound2-dev` (CI already installs it). At runtime
> on WSLg, ALSA routes to PulseAudio - if capture finds no device, install
> `libasound2-plugins` (main.rs already exports PULSE_SERVER for WSLg).

The repo is a **cargo workspace** (root `Cargo.toml`): members are `src-tauri`
(the app), `cli` (terminal helper), and `crates/echo-audio`. Build artifacts are
pinned to `src-tauri/target` via `.cargo/config.toml` so CI paths never move.
Run `cargo test -p echo-audio` / `-p echophrase-cli` / `-p echophrase` from the
repo root.

**`crates/echo-audio` is the single source of truth for audio.** Sample math
(downmix/normalize/peak/RMS), linear + streaming resampling, WAV I/O, live cpal
capture (feature `capture`), and `AudioSource { File(PathBuf) | Memory(AudioBuffer) }` -
the one switch point between disk and memory. Rules:

- Never re-implement resample/downmix/normalize/WAV-read anywhere else; import
  from `echo_audio`.
- Live capture is IN-MEMORY on all platforms (cpal chunks -> `Vec<f32>` 16 kHz
  mono). There are no temp WAVs in the recording path; file input goes through
  `AudioSource::File` (still first-class for file-drop / `transcribe_audio`).
- The Qwen3/ONNX/Candle dispatch lives ONLY in
  `src-tauri/src/transcription/mod.rs::transcribe_source`.
- Open mic (VAD) runs on all platforms; the state machine consumes 256-sample
  chunks from `echo_audio::capture` (Silero v5 requirement).

## Hands-Free E2E Test (TTS -> mic -> transcript)

You can verify the full record -> transcribe -> paste pipeline on macOS with no
human interaction, using the speakers as the audio source:

```bash
osascript -e 'tell application "System Events" to key code 105'  # F13 = record hotkey (start)
sleep 1
say -r 170 "Testing the unified in memory audio pipeline, one two three"
sleep 1
osascript -e 'tell application "System Events" to key code 105'  # F13 again (stop + transcribe)
sleep 5
pbpaste   # the app pastes the transcript -> clipboard holds the proof
```

- `key code 105` is F13, the remapped macOS Dictation key (the default mac
  record hotkey; shown as "F5" in the UI - see `macos_dictation.rs`).
- `say` plays through the speakers and the mic picks it up, so REAL audio flows
  through the REAL capture stack (cpal/CoreAudio), not injected samples.
- Assert in-memory behavior by checking stdout has no temp-file lines and no
  new `*.wav` appears under the app data or temp dirs during the cycle.
- Requires: dev app running, mic + accessibility permissions granted to the
  terminal, speakers audible to the mic (don't mute).
- See also `tests/audio/CLAUDE.md` for the audio test fixtures.

## Development Notes

- **Ports**: `DEV_PORT` (default **8420**) is the Vite dev server, `DEV_PORT + 1` (8421) is HMR, and Tauri's `devUrl` follows it. `vite.config.js`, `src-tauri/src/lib.rs` (dev-server monitor) and `scripts/env/load.mjs` all read the one env var; for any non-default value the launcher generates a gitignored `.tauri-dev-port.json` and passes it to `tauri dev --config` (rewrites `devUrl` and the CSP `ws://` entries). **Do not edit ports in tracked files** - a machine that can't bind 8420 sets `DEV_PORT` in its gitignored `env.yaml` (`shared:` or its platform section). Windows Hyper-V silently reserves random port ranges and the reservations MOVE (1420/1421, then 4420/4421, then 8420 on 2026-09-21), so the symptom is a bind failure with nothing visible in `netstat`/`ss`; run `python scripts/check-ports.py` and test with the launcher's own check (Windows bun `net.listen` and a Python bind have disagreed).
- **F5**: Reloads webview in dev mode (registered globally)
- **Browser DevTools**: Right-click → "Inspect" in Tauri window
- **Logging**: Dev logs to stdout + `../logs/Echophrase.log`
- **No API server**: the app is fully local, so there is no API to test against and no CORS allow-list for `DEV_PORT` to satisfy. Any free port works.

## Known Issues

See **[KNOWN_ISSUES.md](KNOWN_ISSUES.md)** for upstream bugs and platform quirks
we work around. Read it before "cleaning up" code that looks pointlessly
defensive - most of those workarounds guard failures that are completely silent
(a blank splash window, dropped sentences in a transcript, dead titlebar
buttons). Add an entry whenever you work around a bug you do not own.

## Coding Guidelines

- **Never use em dashes (`—`) anywhere.** Always replace with a hyphen `-` or reword. Applies to locale strings, code comments, doc strings, commit messages, PR descriptions, everything. Em dashes cause shell encoding issues and the project explicitly bans them. If you catch yourself typing one, stop and use `-`.
- **Same ban covers the en dash (`–`)** - the medium-length one used traditionally for numeric
  ranges (`2020–2024`, `$89–$99`). It has the same encoding problem and is easier to emit by
  accident, since it looks almost like a hyphen. Write ranges with a plain hyphen: `2020-2024`.
  The regular hyphen `-` is always correct and is the preferred dash in all our copy.
  Grep for both at once: `grep -rn $'—\|–' <paths>`.
- Use forward slashes in paths for cross-platform compatibility
- Conventional commits for both commits and PR titles: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`
- **Before pushing, ask: would a user notice this?** If yes, add a very brief entry to `CHANGELOG.md` under `## [Unreleased]` (`### Added` / `### Fixed` / `### Improved`) **in the same push** - not as a follow-up commit afterwards. `/deploy` turns `[Unreleased]` into the release notes, so anything missing at push time is missing from the release.
  - Write it for users, not for us: what they will notice, in plain language. No file names, function names, or internal mechanics.
  - **Match bullet length to impact.** Tiny cosmetic fixes get tiny bullets ("Fixed text alignments in the top banner." - not a sentence describing each nudged pixel). Save the longer bullets for features and behavior changes users actually care about. Fold multiple micro-fixes in the same area into one short bullet.
  - Skip it for anything invisible to users: refactors, tests, CI, dev-only tooling (e.g. the Debug panel), internal docs.
  - **No fix/improvement bullets for unreleased features.** If a feature is still in `[Unreleased]`, fixes and polish to it are iteration users never saw a "before" of - fold them into (or just refine) the feature's own `### Added` bullet instead of adding separate `### Fixed`/`### Improved` entries. Fixed/Improved bullets are only for behavior that already shipped in a tagged release.
- When updating `env.yaml`, also update `env.yaml.template`
- Safetensors only for Candle models (no GGUF, no PyTorch .bin)
- All user-facing strings must be localized. When adding or changing any string, update all three of `src/lib/locales/en.json`, `src/lib/locales/zh-TW.json` and `src/lib/locales/zh-CN.json` (a vitest parity test fails on key drift). Use `t('key')` in templates and `translate('key')` in JS.
- **When adding a new Rust dependency** (`cargo add`), check its license against `src-tauri/about.toml`'s `accepted` allowlist (the copyleft guard - `cargo about generate` fails the build if a shipped crate's license isn't listed). If the license is already accepted (MIT, Apache-2.0, BSD, MIT-0, CC0-1.0, etc.), no action needed - attribution text itself is auto-generated from `Cargo.lock` at release time (`bun run licenses:generate`), never hand-written. If it's a new license not yet in the list, confirm it's permissive/notice-only (never add GPL/AGPL/LGPL) before adding it to `accepted`. New frontend (`bun add`) dependencies are covered automatically by `rollup-license-plugin` in `vite.config.js` - no manual step there either.

### Backend Architecture Principles
- **Traits for swappable systems**: VAD, transcription, GPU providers use trait interfaces
- **Platform-specific compilation**: Use `#[cfg(target_os = "...")]` for OS-specific code
- **ort loads ORT dynamically**: `load-dynamic` loads `onnxruntime.dll` at runtime via C API; requires `ORT_DYLIB_PATH` pointing to ORT 1.24.x. Auto-installed on dev by `scripts/ensure-ort.mjs`.
- **Model format flexibility**: Support ONNX, safetensors, potential future formats

## Releases (maintainers)

- **Pushes and PRs never release.** `.github/workflows/release.yml` triggers ONLY on
  `v*` tags and `workflow_dispatch`, so fork PRs never run it or see its secrets.
  `.github/workflows/ci.yml` runs checks/tests on every PR and push.
- All tags release from `main`. A `-beta.N` suffix only changes the build flavor
  (staging vite mode + devtools). Every release publishes to the CrabNebula
  **default** channel. `# TRANSITION:` steps in `release.yml` also publish to the
  legacy `beta`/`stable` channels until old installs migrate.
- **No forced updates.** Installs get the normal in-app update prompt.
- Linux binaries are not published (Linux is community-supported; build from
  source). CI still compiles Linux as a check.

**Download URLs (CrabNebula CDN, default channel, `latest` alias only - version-pinned URLs 404):**

| Platform | URL |
|----------|-----|
| Windows NSIS installer | https://cdn.crabnebula.app/download/imperium42/echophrase/latest/platform/nsis-x86_64 |
| macOS DMG (Apple Silicon) | https://cdn.crabnebula.app/download/imperium42/echophrase/latest/platform/dmg-aarch64 |

Other slug: `msi-x86_64`. Linux aliases (`appimage-x86_64`, `debian-x86_64`) are frozen at v0.9.9.

**Package managers.** winget (`Imperium42.Echophrase`) and Homebrew read immutable,
version-pinned installers that `release.yml` publishes to the public
`imperium42/echophrase-releases` repo; the Windows job then runs
`wingetcreate update --submit` (`continue-on-error` - check the run summary after
every release). Scoop uses the `imperium42/scoop-bucket` bucket.

**Release secrets** (repo Settings -> Secrets and variables -> Actions). When a
signing group is unset, that platform's signing steps are skipped with a
`::warning::` and the build ships unsigned / ad-hoc signed:

| Group | Secrets |
|-------|---------|
| Windows signing (Azure Trusted Signing) | `AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TRUSTED_SIGNING_ENDPOINT`, `AZURE_TRUSTED_SIGNING_ACCOUNT`, `AZURE_TRUSTED_SIGNING_PROFILE` |
| macOS signing + notarization | `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `KEYCHAIN_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` |
| Tauri updater signing | `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` |
| Distribution | `CN_API_KEY` (CrabNebula), `RELEASES_REPO_TOKEN` (contents:write on echophrase-releases), `WINGET_PAT` |
| Telemetry (official builds) | `APTABASE_APP_KEY` |

**Verifying signatures** on a downloaded build:

```pwsh
Get-AuthenticodeSignature .\echophrase-setup.exe | Format-List   # Subject: Imperium42, Status: Valid
```
```bash
spctl --assess --type execute --verbose=4 /Applications/Echophrase.app   # accepted, source=Notarized Developer ID
xcrun stapler validate /Applications/Echophrase.app
```

OV signing (Trusted Signing) does not instantly clear SmartScreen; warnings fade as
download reputation accrues. That is expected.
