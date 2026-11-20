# WSL Development Notes

Hard-won lessons from building a Tauri desktop app that runs on WSL2. This is a living document - update it when you find new quirks.

## Table of Contents

- [Opening URLs in Windows Browser](#opening-urls-in-windows-browser-from-wsl)
- [WSL Interop Internals](#wsl-interop-how-argv-becomes-a-windows-command-line)
- [UNC Path Issues](#unc-path-issues)
- [File Explorer / Path Conversion](#opening-directories-in-windows-explorer)
- [Clipboard](#clipboard)
- [Audio / Microphone](#audio-and-microphone)
- [GPU Passthrough](#gpu-passthrough)
- [DOSKEY Warnings](#doskey-warnings)
- [WSL Detection](#wsl-detection)

---

## Opening URLs in Windows Browser from WSL

This is deceptively tricky. Every "obvious" approach has a specific failure mode.

### Why we use cmd.exe: the user's real browser

We intentionally launch URLs through Windows (not a Linux browser): the user's default Windows browser, with their bookmarks and sessions, is where links (docs, Sponsors, GitHub) should open, not a fresh Linux browser session.

### The correct approach: `cmd.exe /c start` with caret escaping

```rust
// Escape cmd.exe metacharacters, then pass as separate args
let escaped = url
    .replace('&', "^&")
    .replace('|', "^|")
    .replace('<', "^<")
    .replace('>', "^>");

Command::new("cmd.exe")
    .arg("/c")
    .arg("start")
    .arg("")          // empty window title (required - see below)
    .arg(&escaped)
    .current_dir("/mnt/c")  // avoid UNC path error (see below)
    .status()
```

**Implementation:** `src-tauri/src/commands/recording.rs` - `open_url_wsl()`

### Why each part matters

**Caret escaping (`^&`):**
WSL's interop converts Rust's argv into a Windows command line using MSVC quoting rules. MSVC only double-quotes args that contain spaces or tabs. URLs rarely have spaces, so `&` passes through unquoted and cmd.exe interprets it as a command separator (e.g. `https://example.com?a=1&b=2` becomes two commands). The `^` character is cmd.exe's escape character - `^&` makes `&` literal.

**Separate `.arg()` calls (not `format!()`):**
Using `format!("cd /d C:\\ && start \"\" \"{}\"", url)` as a single string means the embedded `\"` quotes must survive WSL's argv-to-cmdline conversion. WSL uses MSVC quoting which escapes `"` as `\"`, but cmd.exe doesn't recognize `\"` as an escape sequence - backslash is not special in cmd.exe. The quotes get corrupted in transit. Separate `.arg()` calls let WSL handle each arg independently.

**Empty title arg (`""`):**
cmd.exe's `start` command treats the first quoted argument as a window title. Without the empty string, `start "https://..."` interprets the URL as the window title and then finds nothing to open. `start "" "https://..."` sets an empty title and opens the URL.

**`.current_dir("/mnt/c")`:**
When CWD is inside the WSL filesystem (e.g., `/home/user/project`), cmd.exe inherits a UNC path (`\\wsl.localhost\Ubuntu\home\user\project`). cmd.exe does not support UNC paths as working directories and `start` fails with "Windows cannot find '\\\\'". Setting CWD to `/mnt/c` translates to `C:\` on the Windows side.

### Approaches that DON'T work

| Approach | Problem |
|----------|---------|
| `explorer.exe URL` | Opens File Explorer window instead of browser from WSL |
| `powershell.exe -c Start-Process URL` | 500ms-2s cold start; `&` interpreted as call operator |
| `wslview` (wslu) | Discontinued Feb 2025, not pre-installed, internally uses PowerShell |
| `xdg-open` | Fails in headless WSL2: "no method available for opening" |
| `cmd.exe /c` with `format!()` string | MSVC quoting corrupts embedded `\"` that cmd.exe can't parse |

### Fallback: rundll32

If cmd.exe fails, `rundll32.exe url.dll,FileProtocolHandler` works as a fallback. It uses standard `CommandLineToArgvW` parsing and delegates directly to the URL protocol handler.

```rust
Command::new("rundll32.exe")
    .arg("url.dll,FileProtocolHandler")
    .arg(&url)       // no caret escaping needed - not cmd.exe
    .current_dir("/mnt/c")
    .spawn()
```

### References

- [WSL #3832](https://github.com/microsoft/WSL/issues/3832) - explorer.exe URL handling
- [WSL #2835](https://github.com/microsoft/WSL/issues/2835) - cmd.exe argument quoting from WSL
- [webbrowser-rs](https://github.com/amodm/webbrowser-rs) - Rust crate that uses the same caret escaping approach
- [wslu discontinuation](https://blogs.wedotstud.io/wslu/2025/02/16/discontinued/)

---

## WSL Interop: How argv Becomes a Windows Command Line

When Rust's `Command::new("some.exe")` runs on WSL2, the flow is:

1. Rust builds an argv array on the Linux side
2. WSL's binfmt_misc detects a Windows `.exe` and intercepts the exec
3. WSL converts argv to a flat Windows command-line string using **MSVC quoting rules**
4. The Windows process receives that command line and parses it however it wants

**MSVC quoting rules** (what WSL uses):
- Args containing spaces, tabs, or `"` get wrapped in double-quotes
- Internal `"` characters are escaped as `\"`
- Backslashes before `"` are doubled
- Everything else passes through unquoted

**The trap:** cmd.exe does NOT use MSVC parsing. It has its own rules where `\` is not an escape character and `^` is. So any approach that needs cmd.exe to see specific quoting from WSL is fighting against two incompatible parsing systems.

**Safe zone:** Programs that use `CommandLineToArgvW` (the standard Windows API, which matches MSVC rules) work correctly from WSL. This includes `rundll32.exe`, most native Windows apps, and anything built with MSVC or MinGW.

---

## UNC Path Issues

WSL mounts the Linux filesystem at `\\wsl.localhost\<distro>\...` on the Windows side. When a Windows process is spawned from WSL, it inherits this UNC path as its CWD.

Many Windows tools choke on UNC paths:
- `cmd.exe` prints a warning and defaults to `C:\Windows`
- `start` fails with "cannot find '\\\\'"
- Some .NET tools have issues with UNC working directories

**Fix:** Always set `.current_dir("/mnt/c")` (or any `/mnt/*` path) when spawning Windows executables from WSL. This translates to a normal Windows drive letter path.

---

## Opening Directories in Windows Explorer

When opening a folder (e.g., "Show in Explorer"), we need to convert WSL paths to Windows UNC paths:

```rust
// src-tauri/src/commands/output.rs
let wsl_path = Command::new("wslpath")
    .args(["-w", &linux_path])    // -w = convert to Windows path
    .output()?;
// "/home/user/files" -> "\\wsl.localhost\Ubuntu\home\user\files"

Command::new("explorer.exe")
    .arg(&windows_path)
    .spawn()?;
```

`wslpath` is a WSL built-in utility. The `-w` flag converts Linux paths to Windows UNC paths. Explorer.exe handles UNC paths fine for directory browsing (unlike cmd.exe for CWD).

**File:** `src-tauri/src/commands/output.rs`

---

## Clipboard

WSL2's Wayland clipboard doesn't bridge to Windows. If you copy text inside the Linux app, it's not available in Windows apps (and vice versa).

We have a `copy_to_windows_clipboard` command that pipes text to `clip.exe`:

```rust
// src-tauri/src/commands/recording.rs
let mut child = Command::new("clip.exe")
    .stdin(Stdio::piped())
    .spawn()?;

if let Some(mut stdin) = child.stdin.take() {
    stdin.write_all(text.as_bytes())?;
}
```

`clip.exe` is a Windows built-in that reads from stdin and copies to the Windows clipboard. This is the only reliable way to get text from WSL into the Windows clipboard.

**Status:** Command is registered but not actively used in the frontend yet (Tauri's clipboard plugin may handle it in some configurations).

---

## Audio and Microphone

### PulseAudio passthrough

WSL2 with WSLg provides audio passthrough via PulseAudio at `/mnt/wslg/PulseServer`. We detect this:

```rust
fn is_wsl_audio_available() -> bool {
    std::path::Path::new("/mnt/wslg/PulseServer").exists()
}
```

Audio output (playback) generally works. Recording is more complex.

### Microphone limitation

Tauri's mic-recorder plugin uses ALSA, which is not compatible with WSLg's PulseAudio bridge. Live microphone recording does not work on WSL.

The app detects this and shows an appropriate error message directing users to file upload for transcription.

**Files:**
- `src-tauri/src/commands/transcription.rs` - PulseAudio device enumeration
- `src/lib/composables/useRecording.ts` - WSL-specific error messages

---

## GPU Passthrough

Echophrase runs **CPU-only** on Linux/WSL; no CUDA toolkit is needed to build or run. WSL2
does pass the Windows NVIDIA driver through (`nvidia-smi` works inside WSL), and the GPU
probe uses NVML for VRAM detection. See [gpu-and-cuda.md](gpu-and-cuda.md) for why there is
no CUDA path.

---

## DOSKEY Warnings

When spawning `cmd.exe` from WSL, you may see:
```
'DOSKEY' is not recognized as an internal or external command
```

This is harmless. It comes from Windows AutoRun registry keys (`HKCU\Software\Microsoft\Command Processor\AutoRun`) that try to set up command aliases. The DOSKEY macro file isn't accessible from the WSL-spawned cmd.exe context. These warnings don't affect functionality.

---

## WSL Detection

### Backend (Rust)

Multiple detection methods for reliability (`src-tauri/src/commands/recording.rs`):

```rust
fn is_wsl() -> bool {
    // Any of these indicate WSL
    std::env::var("WSL_DISTRO_NAME").is_ok()
        || std::env::var("WSL_INTEROP").is_ok()
        || std::path::Path::new("/mnt/wslg/PulseServer").exists()
}
```

Also: `std::path::Path::new("/usr/lib/wsl/lib").exists()` used in some places.

### Frontend (TypeScript)

Platform is detected once at startup and cached in a singleton store:

```typescript
// src/lib/stores/platformCache.ts
const cache = platformCache;  // 'wsl' | 'windows' | 'macos' | 'linux' | 'unknown'

// Initialized in useAppInit.ts via get_system_info Tauri command
initPlatformCache(os, isWsl);
```

All subsequent platform checks read from cache (no IPC round-trip).

### Frontend URL dispatch

```typescript
// src/lib/utils/launchUrl.ts
switch (platformCache.platform) {
    case 'wsl':     await invoke('open_url_wsl', { url });     break;
    case 'windows': await invoke('open_url_windows', { url }); break;
    case 'macos':   await invoke('open_url_mac', { url });     break;
    case 'linux':   await invoke('open_url_linux', { url });   break;
}
```

---

## Quick Reference: WSL Workarounds

| Problem | Workaround | File |
|---------|-----------|------|
| Opening URLs in Windows browser | `cmd.exe /c start` with `^&` escaping + `/mnt/c` CWD | `recording.rs` |
| Microphone not available via ALSA | Error message; use file upload instead | `useRecording.ts` |
| Clipboard doesn't bridge to Windows | `clip.exe` via stdin pipe | `recording.rs` |
| WSL paths in Windows Explorer | Convert with `wslpath -w` | `output.rs` |
| UNC CWD breaks cmd.exe/start | `.current_dir("/mnt/c")` on all Windows process spawns | everywhere |
| PulseAudio audio passthrough | Detect `/mnt/wslg/PulseServer`, set `PULSE_SERVER` env | `transcription.rs` |
