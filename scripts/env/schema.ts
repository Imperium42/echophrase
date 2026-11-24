// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Zod schema for env.yaml validation
 * Ensures configuration structure is valid before loading
 *
 * Note: Platform sections contain only platform-specific overrides.
 * Shared fields live in the `shared` section and are merged at runtime by
 * load.mjs. Every object is .passthrough(), so extra keys are exported as-is.
 * There is no CUDA (see https://docs.echophrase.com/developers/decisions/0001-no-cuda), so no CUDA keys are declared.
 */

import { z } from 'zod';

/**
 * Shared configuration used across all platforms
 */
const SharedConfigSchema = z.object({
  // Performance Settings
  OMP_NUM_THREADS: z.string().regex(/^\d+$/, 'Must be a number string').optional(),

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

  // Runtime library search path (keeps /usr/lib/wsl/lib for the GPU probe)
  LD_LIBRARY_PATH: z.string().optional(),

  // Audio Configuration
  PULSE_SERVER: z.string(),
}).passthrough();

/**
 * Windows-specific configuration (platform-specific keys only)
 */
const WindowsConfigSchema = z.object({}).passthrough();

/**
 * Linux-specific configuration (platform-specific keys only)
 */
const LinuxConfigSchema = z.object({}).passthrough();

/**
 * macOS-specific configuration
 */
const MacOSConfigSchema = z.object({
  // Performance
  OMP_NUM_THREADS: z.string().regex(/^\d+$/, 'Must be a number string').optional(),
}).passthrough();

/**
 * Root env.yaml schema
 */
// Sections are nullish: a platform block with every key commented out (e.g. `windows:`) parses
// as YAML null, and that must not fail validation on a fresh clone.
export const EnvConfigSchema = z.object({
  shared: SharedConfigSchema.nullish(),
  wsl: WSLConfigSchema.nullish(),
  windows: WindowsConfigSchema.nullish(),
  linux: LinuxConfigSchema.nullish(),
  macos: MacOSConfigSchema.nullish(),
});

export type EnvConfig = z.infer<typeof EnvConfigSchema>;
export type SharedConfig = z.infer<typeof SharedConfigSchema>;
export type WSLConfig = z.infer<typeof WSLConfigSchema>;
export type WindowsConfig = z.infer<typeof WindowsConfigSchema>;
export type LinuxConfig = z.infer<typeof LinuxConfigSchema>;
export type MacOSConfig = z.infer<typeof MacOSConfigSchema>;
