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
- **Anonymous usage telemetry (removed 2026-11-23, see below)**

## Telemetry removal (done)

Earlier open-source builds still shipped optional anonymous usage stats (Aptabase) in official
builds only, behind the `official` Cargo feature. On 2026-11-23 the maintainers removed it
completely so the network policy above is the whole story:

- **Rust:** `src-tauri/src/telemetry/`, its call sites, the two telemetry commands, the
  `APTABASE_APP_KEY` handling in `build.rs` and the `official` Cargo feature are gone. The
  `telemetry_enabled` setting is dropped; old settings files still load.
- **Frontend:** the onboarding consent checkbox, the Settings > Data toggle,
  `src/lib/ipc/telemetry.ts` and the related strings in all locales are gone.
- **CI and config:** `APTABASE_APP_KEY` and `--features official` are removed from the release
  workflow and `package.json`; the key is gone from `env.yaml.template`; `TRADEMARKS.md` no longer
  mentions the feature.
- **Docs:** [TELEMETRY.md](../TELEMETRY.md) is now a short network and privacy page; the README
  and `CLAUDE.md` are updated. The public privacy docs live in the docs site repo.
- Old v0.9.x installs could still send anonymous stats if allowed; updating removes that.
  (`telemetry.echophrase.com` was never in the CSP, so there was nothing to drop there.)

## Remaining before v1.0.0

- Make the repository public, then tag v1.0.0.
