# Known Issues

Upstream bugs and platform quirks we work around in this repo.

Every entry here is a landmine: the workaround looks arbitrary in isolation, and
removing it "because it looks unnecessary" reintroduces a bug that is usually
silent. Before deleting a workaround, check whether the upstream issue is fixed
in the version we actually ship.

Add an entry when you work around a bug you do not own. Include the symptom, the
real cause, where the workaround lives, and what would let us drop it.

| Issue | Area | Status |
|-------|------|--------|
| [data: URL truncated at `#`](#tauri-data-url-truncated-at-the-first-) | Splash | Open upstream |
| [Titlebar buttons dead after show()](#tao-titlebar-buttons-unresponsive-on-linuxwayland) | Windowing | Open upstream |
| [WSLg render tearing](#wslg-dma-buf-renderer-tearing) | Linux/WSL | Environment workaround |
| [ORT static linking ABI](#ort-msvc-stl-abi-coupling-when-statically-linked) | Transcription | Resolved by load-dynamic |
| [Parakeet TDT drops sentences](#parakeet-tdt-collapses-tokens-without-timestampmodewords) | Transcription | Workaround required |
| [cpal pulls a conflicting windows-core](#cpal-can-resolve-a-conflicting-windows-core) | Windows build | Held by Cargo.lock only |
| [Tauri: new resource files not copied to dev exe](#tauri-new-resource-files-are-not-copied-next-to-the-dev-exe) | Windows dev | Workaround required |

---

## Tauri: data: URL truncated at the first `#`

**Upstream:** [tauri-apps/tauri#15966](https://github.com/tauri-apps/tauri/issues/15966) (filed 2026-09-03, open)
**Affects:** tauri 2.11.5. Any `data:` URL webview in an app that declares a CSP.

**Symptom.** The splash window opens as a blank white rectangle. No error, no
warning, nothing in the log.

**Cause.** When `app.security.csp` is set, Tauri intercepts `data:` URLs in
`manager/webview.rs` (~L484-497): it decodes the base64, injects the CSP, then
re-serializes with `url.set_path(&format!("{},{}", mime::TEXT_HTML, document.html()))`.
The HTML goes back in raw, so the first `#` in the markup starts the URL
*fragment* and the webview receives a document truncated at that byte. A hex
color at byte 86 cut our splash off mid-`<style>`.

**Workaround.** No `#` anywhere in `SPLASH_HTML`: `rgb()` colors, and CSS
`linear-gradient` bars instead of an SVG needing `fill="url(#g)"`.

**Enforced by:** `splash_html_has_no_hash` in `src-tauri/src/setup/splash.rs`.

**Drop it when:** Tauri percent-encodes that path (or keeps the base64). Re-check
`manager/webview.rs` on every Tauri upgrade. Then the SVG logo can come back.

---

## tao: titlebar buttons unresponsive on Linux/Wayland

**Upstream:** [tao#1046](https://github.com/tauri-apps/tao/issues/1046), [tauri#11856](https://github.com/tauri-apps/tauri/issues/11856)

**Symptom.** Minimize / maximize / close do nothing after a window is created
hidden and later shown.

**Cause.** GTK client-side decorations do not recalculate hit regions after a
`visible(false)` to `show()` cycle.

**Workaround.** Toggle resizability across two GTK idle cycles to force a
recalculation, pinning the size before and after to avoid a visible bounce.

**Enforced by:** `src-tauri/src/setup/window.rs` (see the `#[cfg(target_os = "linux")]`
block in `show_main_window`).

---

## WSLg DMA-BUF renderer tearing

**Symptom.** Settings rows visibly vanish during a rapid pointer sweep plus
scroll. Looks like a CSS hover bug. It is not.

**Cause.** WebKitGTK's DMA-BUF renderer tears and flickers under WSLg.

**Workaround.** `WEBKIT_DISABLE_DMABUF_RENDERER=1` forces the legacy buffer path.

**Enforced by:** the `wsl:` block in `env.yaml` / `env.yaml.template`. Linux and
WSL only, inert elsewhere.

---

## ORT: MSVC STL ABI coupling when statically linked

**Upstream:** [pykeio/ort#361](https://github.com/pykeio/ort/issues/361)

**Symptom.** ABI mismatches on Windows when ONNX Runtime is linked statically.

**Workaround.** The `ort` crate uses the `load-dynamic` feature, loading
`onnxruntime.dll` / `.so` / `.dylib` at runtime through the C API. This is why
`ORT_DYLIB_PATH` must be set, and why `scripts/ensure-ort.mjs` exists.

**Enforced by:** `src-tauri/Cargo.toml` (see the comment above the `ort` dependency).

---

## DirectML: the copy in System32 is too old for ORT 1.24

**Upstream:** [DirectML version history](https://learn.microsoft.com/en-us/windows/ai/directml/dml-version-history),
[microsoft/DirectML Releases.md](https://github.com/microsoft/DirectML/blob/master/Releases.md),
real-world report: [lightonai/next-plaid#204](https://github.com/lightonai/next-plaid/issues/204)

**Symptom.** Windows ships its own `C:/Windows/System32/DirectML.dll`, but it lags far
behind the version ORT's DirectML build is made against. If `DirectML.dll` is not next to
`onnxruntime.dll`, ORT loads the inbox copy and ops fail (next-plaid#204: Win11 22H2/23H2
loaded inbox 1.8.0 and `MatMulIntegerToFloat` failed with `0x80070057`).

| Windows build | Inbox DirectML |
|---|---|
| Win10 1903 (18362) | 1.0.0 |
| Win10 2004 (19041) | 1.1.0 |
| Win11 21H2 (22000) | 1.6.0 |
| Win11 22H2 / 23H2 (22621 / 22631) | 1.8.0 |
| Win11 24H2 (26100) | 1.13.1 |
| 1.15.1-1.15.4 | Redistributable (NuGet) only, never inbox |

Per Microsoft's version history, 1.15.4 is "Redistributable only | Microsoft.AI.DirectML.1.15.4".
The `Microsoft.ML.OnnxRuntime.DirectML` 1.24.4 nuspec depends on `Microsoft.AI.DirectML` 1.15.4.
So dropping older Windows versions does not help: even current Win11 (24H2) is too old.

**Workaround.** Bundle the NuGet `DirectML.dll` (1.15.4, ~18.5 MB) next to `onnxruntime.dll` and
preload it by absolute path before ORT starts. Do not "save installer size" by relying on System32.

**Enforced by:** `scripts/ensure-ort.mjs` (`directml` package pin) and
`src-tauri/src/setup/ort_runtime.rs::preload_ort_providers`.

---

## Parakeet TDT collapses tokens without `TimestampMode::Words`

**Symptom.** Whole sentences silently disappear from a transcript. The classic
example: "He saves the world with his brother, Luigi" transcribes as "Luigi".

**Cause.** Without word timestamps the TDT decoder's beam search collapses
tokens.

**Workaround.** Always pass `TimestampMode::Words` to `transcribe_samples()`.

**Enforced by:** `src-tauri/src/transcription/onnx_backend.rs` (the comment and the
`transcribe_samples()` call in the parakeet transcribe path). Nothing fails if you drop it, so leave the comment in place.

---

## cpal can resolve a conflicting windows-core

**Status:** not currently broken, and not verified in this tree. Recorded as a
trap rather than an active bug - see the caveat below before acting on it.

**Symptom (reported).** Windows build failure from two incompatible
`windows-core` versions in the graph. Only shows up on **release-tag Windows
builds** - Linux dev and ordinary PR CI stay green, so it surfaces at the worst
possible moment.

**Cause (reported).** `cpal` 0.18.1 can resolve `windows-core` to 0.62.x, which
conflicts with the 0.61.x that `windows` 0.61.3 expects.

**Current state.** `Cargo.lock` resolves `windows-core` 0.61.2 next to
`cpal` 0.18.1, so the graph is consistent today. There is **no explicit pin in
any `Cargo.toml`**, and no commit has ever added one - the constraint is held
purely by the committed lockfile.

**What that means.** A `cargo update`, a lockfile regeneration, or a dependency
bump that re-resolves `windows-core` could reintroduce the conflict. If a
Windows release build suddenly fails on `windows-core`, check
`cargo tree -d -p windows-core` first, and pin `windows-core = "0.61.2"` in
`src-tauri/Cargo.toml` if two majors show up.

**Caveat.** The failure itself has not been reproduced here - this entry is
written from a prior report plus the lockfile state that matches it. Treat the
mechanism as plausible, not confirmed.

## Tauri: new resource files are not copied next to the dev exe

**Symptom.** Windows dev logs `[ort] bundled DirectML runtime missing: ...target/debug/resources/ort/directml/onnxruntime.dll`,
then ORT loads Windows' old inbox `C:\Windows\System32\onnxruntime.dll` by bare name and the
model preload hangs. Nothing errors.

**Cause.** `tauri-build` copies `bundle.resources` into `target/<profile>/` only when its build
script re-runs, and it only tracks files that existed last time. DLLs that `scripts/ensure-ort.mjs`
stages into `src-tauri/resources/ort/directml/` after a first build never reach `target/debug/`.

**Workaround.** In debug builds `select_ort_runtime()` (`src-tauri/src/setup/ort_runtime.rs`) falls
back to `CARGO_MANIFEST_DIR/resources/ort/directml/onnxruntime.dll`. Release builds are unaffected
(the bundler copies resources at package time). Don't remove the fallback.
