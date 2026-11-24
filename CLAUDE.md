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
bun run test:run                   # Frontend tests (single run; vitest on Node, not --bun - see vitest.config.js)
bun run test:chunked               # Same, in 5-file chunks with a 2min hard timeout each (names a hung chunk)
bun run test:rust                  # All Rust backend tests
bun run test:transcription         # Transcription test (WSL/Linux)
bun run test:transcription:win     # Transcription test (Windows DirectML - needs native Windows path)
bun run test:vad                   # VAD tests

```

> **Agents: never run test suites or `bun run check` in the foreground.** They
> can take 10+ minutes and block the session (`bun run check` + `test:run` hit a
> 10-minute timeout on 2026-11-21). Always run them in the background, and tell
> the user BEFORE starting one, so they know the session is waiting on it.
> Prefer `bun run test:chunked` for the frontend suite: a hang is capped at 2
> minutes per chunk and the hung files are named. Never pipe a suite through
> `tail` (it buffers everything, so a hang looks like silence).

```bash
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
- **GPU**: DirectML (Windows), CoreML (macOS); Linux/WSL is CPU-only. There is no CUDA. ORT is loaded dynamically via ort `load-dynamic`
- **No server, no account**: since the v1.0.0 open-source move, the app is fully free and local. There is no login, tier, trial, payment, heartbeat, MOTD or server cloud sync. The only network calls are model downloads (huggingface.co) and the CrabNebula updater (cdn.crabnebula.app). There is no telemetry or analytics (removed 2026-11-23); see [TELEMETRY.md](TELEMETRY.md).
- **Icons**: import `<Icon>` from `$lib/icons` only - never directly from `@lucide/svelte` or `simple-icons`. Add new icons to `src/lib/icons/registry.ts` (Lucide v1 for functional, Simple Icons for brand logos).
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
├── candle_whisper.rs   # Candle backend (Whisper safetensors; Metal on macOS, CPU elsewhere)
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
4. **Preprocessing**: Audio → 16kHz mono → mel spectrogram (128 bins for Parakeet and Qwen3-ASR; disabled Candle Whisper takes 80 or 128)
5. **Inference**: GPU-accelerated (DirectML on Windows, CoreML/Metal on macOS), CPU on Linux/WSL
6. **Output**: Transcription → clipboard or paste-at-cursor

### Model Support

| Backend | Models | Format | GPU Provider | VRAM |
|---------|--------|--------|--------------|------|
| parakeet-rs | NVIDIA Parakeet-TDT 0.6B v3 (default), Parakeet Lite 120M | ONNX (TDT decode) | DirectML/CoreML/CPU | 2GB |
| Qwen3-ASR | Qwen3-ASR 0.6B | ONNX | DirectML/CoreML/CPU | Varies |
| Candle | Whisper (disabled: `ONNX_ONLY_MODE = true` in `src/lib/types/models.ts`; registry entry kept as a template) | safetensors | Metal/CPU | 2-6GB |
| ONNX (generic) | Any ONNX model | ONNX | DirectML/CoreML/CPU | Varies |

Models auto-download to `~/.cache/huggingface/hub/` on first use with progress streaming.

**Parakeet TDT Gotcha**: Always use `TimestampMode::Words` when calling `transcribe_samples()`. Without it, the TDT decoder's beam search can collapse tokens, causing entire sentences to be dropped (eg "He saves the world with his brother, Luigi" → "Luigi"). See the comment above the `transcribe_samples()` call in `onnx_backend.rs`.

**Memory guards (Parakeet TDT)**: encoder working memory grows much faster than clip length and ORT keeps the peak, so a single 300s call pushed a real Windows install to ~4.5 GB (2026-11-14). Two guards: `transcription/chunking.rs` splits audio over 40s into ~30s chunks cut at the quietest frame near each boundary (clips up to 40s are still one call, text untouched), and `transcription/idle.rs` can unload the model after N idle minutes (OPT-IN: Settings > Optimization, `idle_unload_enabled` default false, `idle_unload_minutes` default 10, clamped 1-120, local-only; never during Recording/Transcribing; the next recording start reloads it in the background, ~7s warm). Measured numbers and the repro method: `BENCHMARKS.md`. Qwen3-ASR and Candle are NOT chunked yet.

**Model Preload Baselines** (`from_pretrained` wall time, for spotting regressions):

| Model | Provider | GPU | Shader cache | Preload time |
|-------|----------|-----|--------------|--------------|
| parakeet-tdt-0.6b-v3 | DirectML | RTX 5080 | warm | ~7.1s |

> **Warm vs cold matters more than the number.** DirectML JIT-compiles shaders on the *first* run for a given GPU architecture and caches them per machine. Warm-cache reloads (above) are fast. A cold first-run on a new GPU arch can take far longer - this is exactly why the Rust-side preload timeout is 300s and why a fast dev reload does **not** prove the cold-JIT deadlock path is healthy. Treat ~7s as the warm regression baseline, not the worst case. See [model load baselines](https://docs.echophrase.com/developers/deep-dives/model-load-baselines).

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
- Platform-specific GPU providers (DirectML, CoreML, NPU)
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

### Network and privacy

There is no telemetry. Do not add any analytics, crash reporting or phone-home call; the only
allowed destinations are listed in [TELEMETRY.md](TELEMETRY.md). Changing that list means
updating that file, the README privacy section and the public privacy docs in the same PR.

## GPU Requirements

> **Verify against the code.** Docs drift; when this file and the code disagree,
> the code wins - fix the doc in the same change.

> **There is no CUDA.** Nothing CUDA-related is needed to build or run, and no CUDA
> code remains in the tree. macOS requires Apple Silicon (M1+, no Intel). Windows
> builds run on bundled DirectML.

### Linux/WSL (CPU only)
- Transcription runs on CPU. `ort` uses `load-dynamic`, so ORT is loaded at runtime
  from `ORT_DYLIB_PATH`; nothing GPU-related is linked or downloaded at build time.
- Dev builds run `--no-default-features`; `devtools` is the only optional cargo feature.
- GPU acceleration on Linux is a wanted future feature (see the TODO in
  `env.yaml.template`).

Why there is no CUDA, and what was removed: [ADR 0001: no CUDA](https://docs.echophrase.com/developers/decisions/0001-no-cuda).

### Windows (DirectML)
- **Windows 11 is the officially supported version.** Windows 10 should work (same build,
  bundled DirectML) but is not tested or officially supported - say exactly that in user-facing copy.
- **DirectML**: DirectX 12 compatible GPU (NVIDIA, AMD, Intel). No additional
  drivers required. Bundled with installer (~15MB).
- Automatic fallback to CPU if GPU unavailable
- Runtime selection lives in `src-tauri/src/setup/ort_runtime.rs::select_ort_runtime()`.

### macOS (Apple Silicon)
- Apple Silicon required (M1/M2/M3/M4) - Intel Macs not supported
- macOS 13.3+ (`minimumSystemVersion` in `tauri.conf.json5`; ORT 1.24 needs C++20 `std::to_chars`)
- Metal GPU acceleration via ONNX Runtime

### ONNX Runtime (load-dynamic, auto-installed for dev)

ort 2.0.0-rc.12 uses `load-dynamic` to load `onnxruntime.dll`/`.so`/`.dylib` at runtime
via the C API. This avoids MSVC STL ABI coupling from static linking (pykeio/ort#361).

- **One stager, all platforms**: `scripts/ensure-ort.mjs` is the ONLY place ORT versions and
  NuGet SHA256s live (ORT 1.24.4, DirectML 1.15.4 = what the ORT DirectML nuspec depends on).
  It runs before `bun run tauri:dev*` and `tauri:build*` (and `bun run setup:ort` standalone);
  release CI runs `bun scripts/ensure-ort.mjs --ci` (stage only, no env.yaml edits). It verifies
  each package hash before extracting (mismatch = loud failure, nothing staged) and is a fast
  no-op when the staged files still match the stamp in `.ort/stamps/`.
- **Windows (DirectML, x64 + arm64)**: stages `onnxruntime.dll`, `onnxruntime_providers_shared.dll`
  and `DirectML.dll` into `src-tauri/resources/ort/directml/` - the bundled path
  `select_ort_runtime()` probes in production, and Tauri copies it next to the dev exe, so dev
  needs no `ORT_DYLIB_PATH`. A placeholder `ORT_DYLIB_PATH` in env.yaml's
  `windows:` section is removed; any other value is kept as a manual override (a warning is
  printed if its path does not exist). `DirectML.dll`
  is bundled rather than taken from System32 because even current Win11 ships a too-old inbox copy
  (24H2 = 1.13.1; 1.15.x is redistributable-only - see KNOWN_ISSUES.md). It is
  proprietary (redistributable), see `ATTRIBUTION.md`.
- **Linux/WSL (CPU, x64 + arm64) and macOS (CoreML, arm64)**: `Microsoft.ML.OnnxRuntime` NuGet
  into a gitignored `.ort/<platform>/lib/`, and an absolute `ORT_DYLIB_PATH` is written into the
  matching env.yaml section (manual values there get overwritten; to use a custom build, call
  `bun x tauri dev` directly).
  - **macOS extra step (automatic):** also copies the dylib to
    `src-tauri/macos-frameworks/libonnxruntime.dylib`. `tauri.conf.json5` declares that path
    under `bundle.macOS.frameworks`, and `tauri_build::try_build` validates it EXISTS at compile
    time - so a bare `cargo build` on a fresh clone fails with `Library not found` unless it is
    staged first. Both `.ort/` and `macos-frameworks/` are gitignored.
- **Required system libraries**: none beyond the OS itself (DirectML and CoreML ship with it).
- **EP selection**: ort's `with_auto_device(MaxPerformance)` enumerates devices at session
  creation time and picks the best one (DirectML > CoreML > NPU > CPU). The user can
  override via Settings > GPU Acceleration radio buttons.
- **Runtime selection (Windows)**: `select_ort_runtime()` in `setup/ort_runtime.rs` runs before any ORT
  API is touched and sets `ORT_DYLIB_PATH`. Precedence: dev override from env.yaml, else bundled DirectML.
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
- **`ECHOPHRASE_CONFIG_DIR`** (test-only): relocates the settings directory (`settings.yaml`) so tests never read or write the real user settings. Tests that touch settings state set it to a temp sandbox (see `src-tauri/src/settings/mod.rs`, `CONFIG_DIR_ENV`).
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

- **A consistent UX is the #1 rule.** Before adding any UI element or behavior, find how the app already does it and reuse that exact pattern (copy the markup/classes or the shared component), never invent a new one. Examples: setting explanations always use the shared `<HintTooltip hint=... />` "?" icon (`settings/primitives/HintTooltip.svelte`), never a hand-rolled copy or a native `title=` tooltip (it doesn't show in the webview). Keep it DRY: shared colors live as tokens in `src/app.css` `:root` (eg `--color-hint`), not hex values repeated per component; toggles use `SettingToggle` / `.toggle-switch`; a locked or forced-on toggle must actually refuse clicks, not just look grey.
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
  - Skip it for anything invisible to users: refactors, tests, CI, dev-only tooling (e.g. the Debug panel), dev docs.
  - **No fix/improvement bullets for unreleased features.** If a feature is still in `[Unreleased]`, fixes and polish to it are iteration users never saw a "before" of - fold them into (or just refine) the feature's own `### Added` bullet instead of adding separate `### Fixed`/`### Improved` entries. Fixed/Improved bullets are only for behavior that already shipped in a tagged release.
- **Every breaking change needs an ADR (Architecture Decision Record) in the dev docs**, in the same PR. ADRs live on the docs site under [Developers > Decisions](https://docs.echophrase.com/developers/decisions) (template and numbering there): add `developers/decisions/NNNN-short-title.mdx` (next number, zero-padded) to the docs site and link it from the PR, covering Status, Context, Decision, Consequences (what breaks, who is affected, migration path), and Alternatives considered. Breaking means anything an existing user, script or contributor depends on stops working as before: settings file keys/format, CLI flags or output, MCP tools, the local IPC API, file/cache locations, dropped platforms/OS versions or models, and build requirements. A breaking change users notice also gets its `CHANGELOG.md` entry; the ADR is the "why", not a substitute.
- When updating `env.yaml`, also update `env.yaml.template`
- Safetensors only for Candle models (no GGUF, no PyTorch .bin)
- All user-facing strings must be localized. When adding or changing any string, update all three of `src/lib/locales/en.json`, `src/lib/locales/zh-TW.json` and `src/lib/locales/zh-CN.json` (a vitest parity test fails on key drift). Use `t('key')` in templates and `translate('key')` in JS.
- **When adding a new Rust dependency** (`cargo add`), check its license against `src-tauri/about.toml`'s `accepted` allowlist (the copyleft guard - `cargo about generate` fails the build if a shipped crate's license isn't listed). If the license is already accepted (MIT, Apache-2.0, BSD, MIT-0, CC0-1.0, etc.), no action needed - attribution text itself is auto-generated from `Cargo.lock` at release time (`bun run licenses:generate`), never hand-written. If it's a new license not yet in the list, confirm it's permissive/notice-only (never add GPL/AGPL/LGPL) before adding it to `accepted`. New frontend (`bun add`) dependencies are covered automatically by `rollup-license-plugin` in `vite.config.js` - no manual step there either.

### Backend Architecture Principles
- **Traits for swappable systems**: VAD, transcription, GPU providers use trait interfaces
- **Platform-specific compilation**: Use `#[cfg(target_os = "...")]` for OS-specific code
- **ort loads ORT dynamically**: `load-dynamic` loads `onnxruntime.dll` at runtime via C API; requires `ORT_DYLIB_PATH` pointing to ORT 1.24.x. Staged on every platform by `scripts/ensure-ort.mjs`.
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
