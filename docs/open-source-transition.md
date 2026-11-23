# Open-source transition (v1.0.0)

This page records how Echophrase moved from a commercial app with a Pro tier to a free,
open-source project, what was removed, and what is still left to do before v1.0.0. It is the
public companion to the maintainers' private release checklist, and it is safe to read as a
contributor: no step here needs access to anything outside this repository.

## What Echophrase is now

- **Free and open source** under the [Mozilla Public License 2.0](../LICENSE). Every feature is
  available to everyone. There is no Pro tier, trial, subscription, payment, account or login.
- **Fully local.** Audio is captured in memory and transcribed on your device.
- **No server of its own.** The app does not talk to any Echophrase backend.
- **No forced updates.** The in-app updater offers new versions; it never blocks an old one.
- **Name and logo are trademarks** of Imperium42 LLC, so forks ship under another name
  ([TRADEMARKS.md](../TRADEMARKS.md)).

## Network policy

The desktop app makes exactly two kinds of network request:

| Destination | Why | When |
|---|---|---|
| `huggingface.co` | Download speech and grammar models | First use of a model, or when you pick a new one |
| `cdn.crabnebula.app` | Check for and download app updates | Official builds, update checks |

Nothing else. No telemetry, no analytics, no crash reporting service, no account sync, no
connectivity probe and no release-history fetch. Any change that adds a network destination must
update this table, [TELEMETRY.md](../TELEMETRY.md) and the public privacy docs in the same PR.

## Platforms

| Platform | Support | GPU |
|---|---|---|
| Windows 11 | Official builds (Windows 10 should work, not officially supported) | DirectML, bundled with the app |
| macOS 13.3+ on Apple Silicon | Official builds, signed and notarized | CoreML / Metal |
| Linux | Community-supported, build from source; no official binaries | CPU only |

CUDA was removed entirely; see [gpu-and-cuda.md](gpu-and-cuda.md).

## What was removed

- Pro tier, feature gates, trials, usage metering and upgrade prompts
- Accounts, sign-in (OAuth and magic link), keychain token storage and cloud settings sync
- The backend API client, heartbeat, MOTD, connectivity probe and minimum-version floor
- Payment and checkout integration
- CUDA execution provider, its downloader and its build flags
- Official Linux builds
- **Anonymous usage telemetry (decided 2026-11-23, removal in progress, see below)**

## Telemetry removal (in progress)

Earlier open-source builds still shipped optional anonymous usage stats (Aptabase) in official
builds only, behind the `official` Cargo feature. On 2026-11-23 the maintainers decided to remove
it completely so the network policy above is the whole story. Checklist:

- **Rust:** delete `src-tauri/src/telemetry/`; remove its calls from `lib.rs` (init, first-run
  events, the release panic hook, the two telemetry commands, the exit flush),
  `commands/transcription.rs`, `commands/recording.rs`, `transcription/mod.rs` and
  `settings/mod.rs` (change detection in `save_settings` and `update_settings_patch`); remove the
  `telemetry_enabled` setting while keeping old settings files loadable; remove the
  `APTABASE_APP_KEY` handling in `build.rs` and the `official` feature in `Cargo.toml`; drop
  `telemetry.echophrase.com` from the CSP in `tauri.conf.json5`.
- **Frontend:** remove the onboarding consent checkbox, the Settings > Data toggle,
  `src/lib/ipc/telemetry.ts`, the `telemetry_enabled` field in `useTauriSettings.ts` and the
  related strings in all three locales.
- **CI and config:** drop `APTABASE_APP_KEY` and `--features official` from the release workflow
  and `package.json`; remove the key from `env.yaml.template`; update `TRADEMARKS.md`.
- **Docs:** rewrite [TELEMETRY.md](../TELEMETRY.md) as a short network and privacy page; update
  the README privacy section, `CLAUDE.md` and the CHANGELOG; update the public privacy docs.
- **Verify:** `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`,
  `cargo test --workspace --no-default-features`, `bun run check`, `bun run lint`,
  `bun run test:chunked`; then a repo-wide search for `telemetry`, `aptabase` and `official`
  should only find changelog history.

Until this lands, official builds still contain the old telemetry code (off for every build from
source). This page will be updated when the removal ships.

## Other open items before v1.0.0

- Attribution: list ONNX Runtime and DirectML on the attribution page, add the embedded Whisper
  mel filterbanks (OpenAI, MIT) and the Noto Color Emoji subset (OFL-1.1) to `ATTRIBUTION.md`, and
  ship the ONNX Runtime and DirectML license notices next to the bundled DLLs.
- Docs accuracy fixes: Node.js as a prerequisite for frontend tests, a stale network-call line in
  `CLAUDE.md`, and minor stale comments and scripts (tracked in the CHANGELOG as they land).
- Make the repository public, then tag v1.0.0.
