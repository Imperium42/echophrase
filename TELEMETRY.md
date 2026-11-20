# Telemetry & Data Collection

Echophrase collects minimal, **anonymized** telemetry via [Aptabase](https://github.com/aptabase/aptabase) (self-hosted on our own infrastructure) to improve the product. Consent is requested on first run (checkbox on the onboarding welcome step, pre-checked) and can be changed anytime in **Settings > Data > "Send anonymous app stats"**. Every event respects the setting - there is no exception.

**Official builds only.** Telemetry is compiled in only with the `official` Cargo feature, which our release CI enables (with our `APTABASE_APP_KEY`). Builds from source (`cargo build`, `bun run tauri:dev`, forks) have the feature OFF: every telemetry call is a no-op and nothing is ever sent. To build an official-flavor binary locally: `cargo build --features official` with `APTABASE_APP_KEY` set (or in `env.yaml`).

There are no accounts, so there is no per-user data anywhere.

See our [Privacy Policy](https://echophrase.com/privacy) for full details.

> **⚠ Keep in sync:** the event table below is published publicly at
> [docs.echophrase.com/privacy/telemetry](https://docs.echophrase.com/privacy/telemetry)
> (`privacy/telemetry.mdx` and its locale copies in the docs site's source repo).
> Any event, property, or opt-out behavior change here MUST be mirrored there in the
> same PR - the public page is a consent disclosure, not a nice-to-have.

---

## What We Collect

### Anonymous Telemetry (Opt-Out Available)

Events are queued locally and sent in batches only while the app is idle (at least 10 minutes since the last recording or transcription), on settings save, and on app exit. Echophrase never makes a telemetry request during or immediately after a transcription - an event is a few hundred bytes of metadata, and your audio and transcripts never leave your machine. **Cannot be tied to any user.**

| Event | Properties | When |
|-------|-----------|------|
| `first_launch` | `arch` (x86_64/aarch64) | Very first launch ever on a machine (once, marker file) |
| `app_updated` | `from`, `to` (version strings) | First launch after an update |
| `daily_active` | `recording_mode` (push_to_talk/open_mic), `arch` | App launch (at most once per 24h) |
| `gpu_backend` | `backend` (cuda/directml/cpu/metal), `vram_bucket` (<4GB/4-8GB/8-16GB/>16GB/unknown) | First transcription per session |
| `transcription_completed` | `model`, `transform_used`, `paste_at_cursor`, `remove_filler_words`, `gpu_backend`, `processing_ms` | After transcription |
| `transcription_failed` | `model`, `gpu_backend`, `error_category` | Transcription error |
| `model_changed` | `model` (e.g. "parakeet-tdt-0.6b-v3") | User saves model selection |
| `settings_saved` | `output_to`, `recording_mode`, `remove_filler_words`, `use_numeric_digits`, `transform_enabled`, `language` | User changes tracked settings |
| `app_panic` | `location` (repo-relative "file.rs:line"), `thread` (thread name) | The app crashes (release builds only) |
| `telemetry_disabled` | _(none)_ | User opts out (final ping) |

Platform, OS version, app version, and locale are captured automatically as system properties.

**No opt-out exceptions:** `first_launch` used to bypass the setting (it fired before Settings was reachable). It no longer does. On a fresh install `lib.rs` sends nothing at startup; the `first_launch`/`app_updated`/`daily_active` trio is deferred to the `telemetry_report_first_run` command, which `Onboarding.svelte` invokes *after* persisting the consent checkbox. Upgrades (marker file already present) report at startup as before, since their settings are already on disk.

**Kill switch:** launching with `DO_NOT_TRACK=1` or `ECHOPHRASE_NO_TELEMETRY=1` disables everything regardless of the setting, and deliberately returns *before* the `.first_launch_done` marker is written so a later normal launch still counts.

**Crash reports contain no personal data:** `app_panic` sends only a source-code location relative to our repository (never your file paths) and the internal thread name - the panic message itself is never sent, because it could contain file paths or other user data.

**NOT collected:**
- User ID or any identifier
- IP address (not stored by our Aptabase instance)
- Transcription content or audio
- Recording duration or frequency (`processing_ms` is model processing time, not recording length)
- Device fingerprints

---

## How to Opt Out

**Settings > Data > "Send anonymous app stats"** - toggle off to disable anonymous telemetry. No confirmation prompt (arguing with a decision the user already made is a dark pattern). Or set `DO_NOT_TRACK=1` / `ECHOPHRASE_NO_TELEMETRY=1` in the environment.

Note: Disabling telemetry does not affect app functionality. One final `telemetry_disabled` event is sent when you opt out, then all tracking ceases.

---

## Local-Only Data

The following is stored locally on your device and **never sent to our servers**:

- Audio recordings (deleted immediately after transcription)
- Transcription history (if enabled)
- Settings and preferences
- Model files (cached locally)

---

## Technical Details

- **Backend**: [Aptabase](https://github.com/aptabase/aptabase) (self-hosted)
- **Host**: `https://telemetry.echophrase.com`
- **Dashboard**: `https://telemetry.echophrase.com` (login required)
- **Transport**: HTTPS only (TLS 1.3)
- **Batching**: Events queued locally, flushed every 60s (2s in debug), on settings save, and on app exit
- **Retention**: Raw events deleted after 1 year
- **Source**: `src-tauri/src/telemetry/` (client.rs + mod.rs)
- **No cookies, no fingerprinting, no cross-app tracking**
