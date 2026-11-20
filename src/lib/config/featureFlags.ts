// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Echophrase feature flags.
 *
 * Single source of truth for capabilities that are toggled on/off at the
 * codebase level (NOT per-user, NOT per-tier, NOT runtime-configurable).
 * Flip the flag here, recompile, shipped state changes everywhere.
 */

/**
 * **CUDA Plugin (Windows opt-in DLC for ORT CUDA execution provider).**
 *
 * ## Status: DISABLED everywhere as of 2026-04-15.
 *
 * ## Why disabled
 *
 * The plugin downloads ORT 1.24.2's GPU prebuild
 * (`onnxruntime-win-x64-gpu-1.24.2.zip`) from Microsoft's GitHub releases.
 * Microsoft's stable 1.24.x Windows GPU build is compiled against the
 * **CUDA 12.x** runtime - the provider DLL hardcodes `cudart64_12.dll`,
 * `cublas64_12.dll`, etc as PE imports.
 *
 * Most NVIDIA users in 2026 have **CUDA 13.x** installed (default for new
 * installs; required by RTX 50-series Blackwell GPUs which have `sm_120`
 * compute capability). Those users cannot load the plugin without ALSO
 * installing CUDA 12 alongside their CUDA 13 - which is unacceptable UX.
 *
 * ## Workarounds we evaluated and rejected
 *
 * - **Bundle CUDA 12 runtime DLLs in the plugin (~+200MB)**: would work for
 *   non-Blackwell CUDA-13-only systems, but Blackwell still fails because
 *   the CUDA 12 build lacks `sm_120` GPU kernels. Half-fix.
 * - **Switch to ORT 1.25 nightly with CUDA 13**: nightly access has auth
 *   issues (microsoft/onnxruntime#26568) and we don't ship nightly to users.
 *
 * ## What we are waiting for
 *
 * Microsoft to ship **ORT 1.25 stable Windows GPU prebuild with CUDA 13.x
 * support**. Tracking issues:
 *
 * - https://github.com/microsoft/onnxruntime/issues/26238 (CUDA 13 feature request)
 * - https://github.com/microsoft/onnxruntime/discussions/26586 (CUDA 13 build workaround)
 * - https://github.com/microsoft/onnxruntime/issues/27944 (CUDA 13 testing packages)
 * - https://github.com/microsoft/onnxruntime/releases (watch for 1.25.0 stable)
 *
 * ## When ORT 1.25 stable lands - migration checklist
 *
 * 1. Bump the `ort` crate in `src-tauri/Cargo.toml` to the matching version.
 * 2. Update `PLUGIN_URL` and `PLUGIN_VERSION` in `src-tauri/src/ort_plugin/mod.rs`.
 * 3. Refresh `PLUGIN_SHA256` (compute from the new release's checksum).
 * 4. Flip this flag to `true`.
 * 5. Test the install + load flow on:
 *    - CUDA 12.x system (legacy users)
 *    - CUDA 13.x system (Ada / Hopper / mainstream)
 *    - RTX 50-series with CUDA 13 (Blackwell, sm_120) - the key validation
 * 6. Update `CLAUDE.md` GPU Requirements to reflect the new supported CUDA range.
 * 7. Update `src/lib/locales/en.json` + `zh-TW.json` + `zh-CN.json` "cudaIncompatible" string
 *    if the failure mode changes.
 *
 * ## Linux note
 *
 * The Linux production build links CUDA at compile time (per CLAUDE.md GPU
 * Requirements section), so Linux users get CUDA acceleration without needing
 * the plugin. **This flag specifically gates the Windows DLC plugin UI**, not
 * Linux CUDA support. The CUDA radio button in Settings > GPU Acceleration
 * continues to appear on Linux as today.
 *
 * ## What this flag controls
 *
 * - `GpuAccelerationSection.svelte` hides the CUDA radio button on Windows + Mac
 *   when this is `false` (Mac never had it anyway; Windows is the meaningful change).
 * - The CUDA Setup modal cannot open when this is `false` (no entry point).
 * - The backend `ort_plugin` module + Tauri commands stay compiled and registered
 *   so flipping this flag back to `true` is a one-line change with no Rust touch.
 */
export const CUDA_PLUGIN_ENABLED = false;
