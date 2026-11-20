// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Zod schema for env.yaml validation
 * Ensures configuration structure is valid before loading
 *
 * Note: Platform sections contain only platform-specific overrides.
 * Shared fields (CT2 tuning, etc.) live in the `shared` section and
 * are merged at runtime by load-env.js.
 */

import { z } from 'zod';

/**
 * Shared configuration used across all platforms
 */
const SharedConfigSchema = z.object({
  // CTranslate2 Optimizations
  CT2_CUDA_TRUE_FP16_GEMM: z.enum(['0', '1'], {
    errorMap: () => ({ message: 'Must be "0" or "1"' })
  }),
  CT2_USE_EXPERIMENTAL_PACKED_GEMM: z.enum(['0', '1']).optional(),

  // Performance Settings
  OMP_NUM_THREADS: z.string().regex(/^\d+$/, 'Must be a number string'),
  CUDA_DEVICE_ORDER: z.string().optional(),
  CUDA_VISIBLE_DEVICES: z.string().optional(),

  // OpenGL Software Rendering
  LIBGL_ALWAYS_SOFTWARE: z.enum(['0', '1']).optional(),
}).passthrough();

/**
 * WSL-specific configuration (platform-specific keys only)
 */
const WSLConfigSchema = z.object({
  // WebKitGTK rendering (WSLg tearing/flicker workaround)
  WEBKIT_DISABLE_DMABUF_RENDERER: z.enum(['0', '1']).optional(),
  WEBKIT_DISABLE_COMPOSITING_MODE: z.enum(['0', '1']).optional(),

  // CUDA is disabled on Linux/WSL, so the build-affecting CUDA vars
  // (NVCC_PREPEND_FLAGS, CC, CXX, CUDA_TOOLKIT_ROOT_DIR, CMAKE_CUDA_COMPILER,
  // LIBRARY_PATH, RUSTFLAGS, ORT_CUDA_VERSION) are intentionally NOT declared
  // here. They were removed rather than made .optional() so that every key in
  // this schema still means "genuinely required" - an optional key that nothing
  // reads is a guardrail that silently accepts typos. This object is
  // .passthrough(), so re-adding them to env.yaml exports them again with no
  // schema change needed. See docs/gpu-and-cuda.md.

  // CUDA Paths - runtime only (GPU probe / NVML), harmless to the build.
  CUDA_HOME: z.string(),
  CUDNN_PATH: z.string(),

  // Library Paths. LD_LIBRARY_PATH is runtime (dlopen); its link-time twin
  // LIBRARY_PATH is gone with the rest of the CUDA build vars.
  LD_LIBRARY_PATH: z.string(),
  PATH: z.string(),

  // Model Paths
  CT2_MODEL_DIR: z.string(),
  WHISPER_MODELS_PATH: z.string(),

  // Audio Configuration
  PULSE_SERVER: z.string(),

  // Development Paths
  TAURI_PROJECT_PATH: z.string(),
  CT2RS_PATCH_PATH: z.string().optional(),
}).passthrough();

/**
 * Windows-specific configuration (platform-specific keys only)
 */
const WindowsConfigSchema = z.object({
  // ONNX Runtime build-script CUDA major version override (12 or 13)
  ORT_CUDA_VERSION: z.string().optional(),

  // CUDA Paths
  CUDA_HOME: z.string(),
  CUDA_PATH: z.string(),
  CUDNN_PATH: z.string(),
  CUDA_TOOLKIT_ROOT_DIR: z.string(),
  CMAKE_CUDA_COMPILER: z.string(),

  // Visual Studio paths
  CMAKE_GENERATOR: z.string(),
  CMAKE_GENERATOR_PLATFORM: z.string(),
  CMAKE_SH: z.string(),
  NVCC_APPEND_FLAGS: z.string().optional(),
  CMAKE_ARGS: z.string().optional(),
  CUDA_NVCC_FLAGS: z.string().optional(),
  CMAKE_CUDA_FLAGS: z.string().optional(),
  CC: z.string(),
  CXX: z.string(),
  RUSTFLAGS: z.string(),
  RUSTC_LINKER: z.string(),

  // Visual Studio Environment
  VS170COMNTOOLS: z.string().optional(),
  VSINSTALLDIR: z.string().optional(),
  VCToolsInstallDir: z.string().optional(),
  VCINSTALLDIR: z.string().optional(),
  WindowsSdkDir: z.string().optional(),
  WindowsSDKVersion: z.string().optional(),
  CUDA_PATH_V12_8: z.string().optional(),
  VCVARS_VER: z.string().optional(),
  DevEnvDir: z.string().optional(),
  CMAKE_CUDA_HOST_COMPILER: z.string().optional(),
  CUDAHOSTCXX: z.string().optional(),

  // Library Paths
  PATH: z.string(),
  LIB: z.string(),
  INCLUDE: z.string(),

  // Model Paths
  CT2_MODEL_DIR: z.string(),
  WHISPER_MODELS_PATH: z.string(),

  // Development Paths
  TAURI_PROJECT_PATH: z.string(),
  CT2RS_PATCH_PATH: z.string().optional(),

  // Visual Studio Compiler
  MSVC_VERSION: z.string().optional(),
}).passthrough();

/**
 * Linux-specific configuration (platform-specific keys only)
 */
const LinuxConfigSchema = z.object({
  // CUDA is disabled on Linux too - build-affecting CUDA vars (CC, CXX,
  // CUDA_TOOLKIT_ROOT_DIR, CMAKE_CUDA_COMPILER, LIBRARY_PATH, ORT_CUDA_VERSION)
  // are intentionally NOT declared here. Same reasoning as WSLConfigSchema above;
  // .passthrough() means re-adding them to env.yaml needs no schema change.

  // CUDA Paths - runtime only (GPU probe / NVML), harmless to the build.
  CUDA_HOME: z.string(),
  CUDNN_PATH: z.string(),

  // Library Paths - runtime (dlopen) only.
  LD_LIBRARY_PATH: z.string(),
  PATH: z.string(),

  // Model Paths
  CT2_MODEL_DIR: z.string(),
  WHISPER_MODELS_PATH: z.string(),

  // Development Paths
  TAURI_PROJECT_PATH: z.string(),
}).passthrough();

/**
 * macOS-specific configuration
 */
const MacOSConfigSchema = z.object({
  // Performance
  OMP_NUM_THREADS: z.string().regex(/^\d+$/, 'Must be a number string'),

  // Model Paths
  CT2_MODEL_DIR: z.string(),
  WHISPER_MODELS_PATH: z.string(),

  // Development Paths
  TAURI_PROJECT_PATH: z.string(),
}).passthrough();

/**
 * Root env.yaml schema
 */
export const EnvConfigSchema = z.object({
  shared: SharedConfigSchema.optional(),
  wsl: WSLConfigSchema.optional(),
  windows: WindowsConfigSchema.optional(),
  linux: LinuxConfigSchema.optional(),
  macos: MacOSConfigSchema.optional(),
});

export type EnvConfig = z.infer<typeof EnvConfigSchema>;
export type SharedConfig = z.infer<typeof SharedConfigSchema>;
export type WSLConfig = z.infer<typeof WSLConfigSchema>;
export type WindowsConfig = z.infer<typeof WindowsConfigSchema>;
export type LinuxConfig = z.infer<typeof LinuxConfigSchema>;
export type MacOSConfig = z.infer<typeof MacOSConfigSchema>;
