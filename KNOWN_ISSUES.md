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
| [No ORT CUDA 13 Windows build](#no-onnx-runtime-cuda-13-windows-gpu-prebuild) | GPU | Blocked upstream |
| [CUDA EP preload fails on Linux](#ort-cuda-provider-preload-cannot-work-on-linux) | GPU | Won't fix (by design) |
| [cpal pulls a conflicting windows-core](#cpal-can-resolve-a-conflicting-windows-core) | Windows build | Held by Cargo.lock only |

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

## Parakeet TDT collapses tokens without `TimestampMode::Words`

**Symptom.** Whole sentences silently disappear from a transcript. The classic
example: "He saves the world with his brother, Luigi" transcribes as "Luigi".

**Cause.** Without word timestamps the TDT decoder's beam search collapses
tokens.

**Workaround.** Always pass `TimestampMode::Words` to `transcribe_samples()`.

**Enforced by:** `src-tauri/src/transcription/onnx_backend.rs:374` (comment) and
`:380` (the call). Nothing fails if you drop it, so leave the comment in place.

---

## No ONNX Runtime CUDA 13 Windows GPU prebuild

**Upstream:** [onnxruntime#26238](https://github.com/microsoft/onnxruntime/issues/26238),
[discussion#26586](https://github.com/microsoft/onnxruntime/discussions/26586),
[onnxruntime#27944](https://github.com/microsoft/onnxruntime/issues/27944)

**Symptom.** The Windows CUDA plugin cannot load for most users.

**Cause.** Microsoft's stable ORT 1.24.x Windows GPU build is compiled against
the CUDA 12 runtime and hardcodes `cudart64_12.dll` / `cublas64_12.dll` as PE
imports. Most NVIDIA users are on CUDA 13, which RTX 50-series Blackwell
(`sm_120`) requires. Bundling the CUDA 12 runtime does not fix Blackwell, since
that build lacks `sm_120` kernels.

**Status.** The plugin is disabled everywhere as of 2026-04-15. Windows uses
DirectML, which covers NVIDIA, AMD, Intel and Qualcomm with no extra install.

**Enforced by:** `src/lib/config/featureFlags.ts`, which also carries the
migration checklist for when ORT 1.25 stable ships.

---

## ORT CUDA provider preload cannot work on Linux

**Not an upstream bug** - a consequence of how ELF symbol scoping works. Recorded
here because the failure looks alarming and invites a wrong fix.

**Symptom.** `[ORT preload] failed for libonnxruntime_providers_cuda.so:
undefined symbol: Provider_GetHost` on every launch.

**Cause.** `ort::util::preload_dylib` opens with `RTLD_LOCAL`, so
`libonnxruntime_providers_shared.so`'s symbols stay out of the global namespace.
`providers_cuda.so` does not name `providers_shared` in its `DT_NEEDED` - it
expects those symbols to already be globally visible. Linux only: Windows PE
imports name their source DLL, and macOS ships no CUDA provider.

**Workaround.** The Linux preload list omits `providers_cuda`. Nothing is lost:
ORT loads the CUDA EP itself at session creation via `with_auto_device`.

**Enforced by:** `src-tauri/src/setup/ort_runtime.rs` (see the `TODO(cuda-linux)`
comment on the Linux `next_to_ort` list).

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
