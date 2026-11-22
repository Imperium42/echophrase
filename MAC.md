# MAC.md - macOS Platform Support

## Supported Hardware

**Apple Silicon Macs only (M1 or later, macOS 13+)**

Intel Macs are **not supported**. We tested extensively on a 2017 MacBook Air (macOS 12.6, Intel x86_64) and explored every option:

- **ONNX Runtime crashes** on macOS < 13.3 - the ORT 1.24.x dylib uses `std::to_chars(double)` from C++20, which requires macOS 13.3+ libc++. The `dlopen` fails with `Symbol not found: __ZNSt3__18to_charsEPcS0_d`. We added dlopen probes, `catch_unwind` wrappers, and an `ONNX_RUNTIME_AVAILABLE` flag, but none of this makes ONNX *work* - it just prevents the app from hanging.
- **Candle + Metal crashes** on macOS < 13 - `Device::new_metal(0)` triggers an Objective-C exception from `MTLDevice.architecture` (selector unavailable before macOS 13). We fall back to CPU-only.
- **CPU-only transcription is unusable** - whisper-small takes >5 minutes to transcribe 3 seconds of audio ("test one two three") on the 2017 MacBook Air. No model is fast enough on Intel CPU alone.
- **Building ORT from source** is not feasible for macOS 12 - Apple's SDK headers block `std::to_chars` at compile time for deployment targets < 13.3.

Users on Intel Macs can browse the UI but recording is disabled with an explanatory banner.

## GPU Acceleration (Apple Silicon)

| Backend | API | Notes |
|---------|-----|-------|
| Candle | Metal | `Device::new_metal(0)` - requires macOS 13+ |
| ONNX | CoreML EP | Requires macOS 13.3+ (ORT dylib requirement) |

## WKWebView / Safari Quirks

These affect all Macs, including Apple Silicon on older macOS versions.

### Tailwind v4 Gradient Syntax

Tailwind v4 sets `--tw-gradient-position: to right in oklab` for `bg-gradient-to-r`. The `in oklab` color interpolation syntax requires **Safari 16.2+ (macOS 13.1+)**. On older WKWebView, gradients silently fail and render as transparent/black.

**Fix** (in `src/app.css`):
```css
.bg-gradient-to-r  { --tw-gradient-position: to right; }
.bg-gradient-to-l  { --tw-gradient-position: to left; }
.bg-gradient-to-t  { --tw-gradient-position: to top; }
.bg-gradient-to-b  { --tw-gradient-position: to bottom; }
.bg-gradient-to-tr { --tw-gradient-position: to top right; }
.bg-gradient-to-tl { --tw-gradient-position: to top left; }
.bg-gradient-to-br { --tw-gradient-position: to bottom right; }
.bg-gradient-to-bl { --tw-gradient-position: to bottom left; }
```

### overflow-hidden + will-change Clips box-shadow

On WKWebView, if an element has both `overflow: hidden` and `will-change: transform` (or `transform: translateZ(0)`), the `box-shadow` is clipped and doesn't render outside the element bounds.

**Fix**: Put `overflow-hidden` on an inner wrapper div, keep shadow on the outer element.

### GPU Compositing Suppresses Rendering

A global rule like:
```css
[class*="transition-"], [class*="animate-"] {
  transform: translateZ(0);
  will-change: transform, opacity;
}
```
causes WKWebView composited layers to suppress gradient/shadow rendering. Modern browsers handle GPU promotion automatically - don't force it.

## Development Setup (Apple Silicon)

```bash
bun run tauri:dev:mac
```

The first run downloads ONNX Runtime 1.24.x into a gitignored `.ort/` folder, writes
`ORT_DYLIB_PATH` into your `env.yaml`, and stages the dylib at
`src-tauri/macos-frameworks/libonnxruntime.dylib` (which `tauri build` requires).
To do only that step: `bun run setup:ort`.

## Input Simulation

- Uses **Cmd+V** (Meta key) for paste, not Ctrl+V
- Requires **Accessibility permission** for `enigo` input simulation
- Users must grant permission in System Preferences > Security & Privacy > Privacy > Accessibility

## Minimum macOS Versions

| Feature | Minimum macOS | Notes |
|---------|--------------|-------|
| App (shipped minimum) | 13.3+ | `minimumSystemVersion` in `tauri.conf.json5` |
| Tailwind v4 gradients (oklab) | 13.1+ | Safari 16.2 |
| Metal GPU (Candle) | 13.0+ | MTLDevice.architecture selector |
| ONNX Runtime 1.24.x | 13.3+ | C++20 std::to_chars |
| **Full functionality** | **13.3+** | **Apple Silicon required** |
