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
| [DirectML: System32 copy too old](#directml-the-copy-in-system32-is-too-old-for-ort-124) | Windows runtime | Workaround required |
| [tao panics after Windows ends the session](#tao-cannot-move-state-from-destroyed-after-wm_endsession) | Windows shutdown | Workaround required |
| [Linux: F13-F24 hotkeys don't register](#linux-f13-f24-hotkeys-dont-register-on-a-stock-keymap) | Linux hotkeys | User keymap fix |
| [WSLg: Ctrl+Shift+Space is reserved](#wslg-ctrlshiftspace-is-reserved-by-the-compositor) | WSL hotkeys | Upstream, no fix |
| [Windows: admin windows block paste](#windows-admin-elevated-windows-block-paste-and-injected-hotkeys) | Windows input | OS design, no fix |
| [Windows: tray pin reset once on 1.0](#windows-tray-always-show-pin-reset-once-on-the-update-to-10) | Windows tray | One-time, by design |
| [rollup-license-plugin: license file pick is random](#rollup-license-plugin-picks-a-license-file-in-readdir-order) | Attribution CI | Workaround required |

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

**Code signing note.** Because the needed DirectML is redistributable-only, the installer has to
bundle Microsoft's (closed-source, Microsoft-signed) `DirectML.dll`. We asked SignPath Foundation
whether that counts under their "System Libraries" exception for free OSS signing:
[SignPath/fdn-website#29](https://github.com/SignPath/fdn-website/issues/29) (open, 2026-11-24).
DirectML stays bundled either way.

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

---

## tao: "cannot move state from Destroyed" after WM_ENDSESSION

**Upstream:** [tauri-apps/tao#1310](https://github.com/tauri-apps/tao/issues/1310) (open),
fixed in tao 0.37.0 by [tauri-apps/tao#1157](https://github.com/tauri-apps/tao/pull/1157)
**Affects:** tao 0.35.x (tauri 2.11.x, what we ship). Windows only.

**Symptom.** The app crashes with `panicked at ... tao-0.35.3\src\platform_impl\windows\event_loop\runner.rs:371:
cannot move state from Destroyed` while Windows signs out, shuts down or restarts, or when an
installer's Restart Manager closes the app. Seen in the wild on 0.8.0-beta.74 and 0.9.1.

**Cause.** On `WM_ENDSESSION` tao calls `loop_destroyed()` (moving its runner to `Destroyed`) but
keeps pumping messages. The next event it dispatches calls `move_state_to` again and hits the
`(Destroyed, _) => panic!` arm. None of our quit paths are involved, so `shutdown::force_exit`
never ran.

**Workaround.** `lib.rs` `.run(...)` force-exits on a `RunEvent::Exit` (tao's `LoopDestroyed`)
that was NOT preceded by `RunEvent::ExitRequested`, so the process ends inside the event, before
tao can dispatch another one. Exits with an `ExitRequested` first (tauri `relaunch()` after an
update, `request_restart`) are left alone: tauri restarts only after the `Exit` callback returns,
and force-exiting there would kill every relaunch. Decision logic and tests:
`shutdown::exit_code_for_run_event`.

**Drop it when:** probably never outright. tao 0.37's fix calls `std::process::exit(0)`, which on
Windows still runs DLL detach for ORT/DirectML and can hang (see `shutdown.rs`), so our
`TerminateProcess` path stays the better exit. Re-check on the tauri 2.12 bump (blocked on the
tray-icon GUID fork, see the `[patch.crates-io]` note in the root `Cargo.toml`).

## Linux: F13-F24 hotkeys don't register on a stock keymap

**Upstream:** none (xkb default keymap + `global-hotkey` 0.8 X11 backend design)
**Affects:** Linux X11 (and XWayland). Macro keys, mouse buttons remapped to F13+.

**Symptom.** Recording F15 (or F13-F24) as the hotkey either records the wrong key or saves
"F15" but the hotkey never fires.

**Cause.** The default xkb keymap gives F13+ launcher keysyms instead of `F13`-`F24`
(`/usr/share/X11/xkb/symbols/inet`: `<FK13>` = `XF86Tools`, `<FK15>` = `XF86Launch6`, ...). So the
webview reports the launcher name in `event.key`, and `global-hotkey` (which grabs by looking up
the keycode that holds the `F15` keysym) finds no such keycode and fails to register.

**Workaround.** The recorder takes F1-F24 from the physical `event.code`
(`src/lib/utils/hotkeyKey.ts`), so it records `f15` correctly. Registration still needs the key to
carry the `F15` keysym, which only the user's keymap can provide, eg
`xmodmap -e "keycode 193 = F15"` (keycode = FK15 + 8 on evdev; check with `xev`) in a startup
script. On WSLg the RDP layer may not forward F13+ at all (untested).

**Drop it when:** `global-hotkey` can grab by keycode, or we read raw evdev input on Linux.

## WSLg: Ctrl+Shift+Space is reserved by the compositor

**Upstream:** WSLg's Weston RDP backend (`libweston/backend-rdp/rdprail.c`, pinned commit
`2318feca` in WSLg 1.0.71, see `/mnt/wslg/versions.txt`)
**Affects:** the Linux build under WSL only (dev setups). Native Windows is unaffected.

**Symptom.** The hotkey recorder stays on "Press keys..." for Ctrl+Shift+Space, and that combo
never works as a hotkey. Ctrl+Space and Shift+Space are fine. Logging shows the Control and Shift
keydowns arriving but never the Space.

**Cause.** The RDP backend unconditionally installs Ctrl+Shift+Space as its debug-key trigger
(`weston_install_debug_key_binding(b->compositor, MODIFIER_CTRL)`), so the compositor consumes
the Space before any client sees it. There is no config or env switch. Neither X11 nor Windows
`RegisterHotKey` reports the combo as taken, so our "in use" check can't detect it. After it,
Weston also swallows a following M, W or D (it writes a debug dump to `/mnt/wslg/weston.log`).

**Workaround.** Pick another combo under WSL. Settings > Hotkeys shows a WSL-only note only when it
happens: Ctrl+Shift held and released with no third key during a pick (the swallowed Space looks
exactly like that), or when Ctrl+Shift+Space is the saved hotkey (it is the default).

**Drop it when:** WSLg makes the debug binding optional or removes it.

## Windows: admin (elevated) windows block paste and injected hotkeys

**Upstream:** none (Windows User Interface Privilege Isolation, by design)
**Affects:** Windows, when an app running as administrator (eg an admin terminal) has focus.

**Symptom.** Paste-at-cursor does nothing in an admin window (the text is still on the
clipboard). A hotkey produced by software (mouse/macro-key remapping tools) may not fire at all.

**Cause.** UIPI silently drops simulated input (`SendInput`) from a normal process into a
higher-integrity window. Our paste is a simulated Ctrl+V (`commands/output.rs`, enigo), and
remapping tools generate their keys the same way. Our hotkey uses `RegisterHotKey`, which is
expected to fire for real keyboard presses over admin windows; a dead hotkey there usually means
the key was injected by a non-elevated remapping tool.

**Workaround.** None in-app. Run Echophrase (and/or the remapping tool) as administrator. A
`uiAccess` manifest would avoid it but requires a signed exe installed under Program Files, and we
install per-user to `AppData\Local`.

**Drop it when:** never (OS design), unless the install location changes.

## Windows: tray "always show" pin reset once on the update to 1.0

**Upstream:** none (Windows tray identity rules)
**Affects:** Windows installs that pinned the tray icon on a pre-1.0 version.

**Symptom.** After updating to 1.0.x the tray icon moves to the overflow (hidden icons) even
though it was pinned to the taskbar.

**Cause.** 1.0.0 gave the tray icon a fixed `NOTIFYICONDATA` GUID (`setup/tray.rs`,
`TRAY_ICON_GUID`) so the pin survives future updates and per-version install folders (Scoop).
Windows treats a GUID icon as a new identity, separate from the old exe-path + UID one the pin
was stored on, so it starts unpinned once.

**Workaround.** None needed after one re-pin (Settings > Personalization > Taskbar > Other
system tray icons). We deliberately don't write `IsPromoted` under
`HKCU\Control Panel\NotifyIconSettings` ourselves: undocumented, and it overrides a user choice.

**Drop it when:** pre-1.0 installs are gone.

## rollup-license-plugin picks a license file in readdir order

**Upstream:** `rollup-license-plugin` 3.x (`getLicenseFileName` in `dist/index.js`)
**Affects:** `bun run licenses:generate` and the CI "Attribution is current" check.

**Symptom.** "Attribution is current" fails intermittently with no dependency change: the
notice block for a dual-licensed npm package (`@tauri-apps/api`, `Apache-2.0 OR MIT`) flips
between the Apache-2.0 and MIT text, and a re-run passes.

**Cause.** The plugin returns the first `LICENSE*` file in `readdir()` order, which is not
sorted and varies by filesystem and install. `@tauri-apps/api` ships both `LICENSE_APACHE-2.0`
and `LICENSE_MIT`.

**Workaround.** `vite.config.js` passes `pickLicenseFile` (alphabetically first `LICENSE*`
file) through the plugin's typed `__mocks__.getLicenseFileName` hook, the only override it
exposes. Gated to the attribution build (`GEN_ATTRIBUTION=1`).

**Drop it when:** the plugin sorts its candidates or offers a real option for this.
