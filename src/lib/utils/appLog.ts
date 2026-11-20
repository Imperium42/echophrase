// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Forward a message to the Rust log (Echophrase.log + stdout in dev).
 *
 * `console.log` from the webview does NOT reach the app log - only
 * console.error/warn are forwarded by the dev server, so ordinary
 * `console.log` tracing is invisible when debugging from logs alone.
 * Use this for state transitions worth seeing in a log file (model
 * selection, settings changes) while the state itself stays in memory.
 *
 * Fire-and-forget: logging must never break the calling flow.
 *
 * @example
 * appLog('[ModelStore] selected qwen3-asr-0.6b (memory)');
 */
import { invoke } from '@tauri-apps/api/core';

/** tauri-plugin-log level values. */
const LOG_LEVEL_INFO = 3;
const LOG_LEVEL_WARN = 4;

function send(level: number, message: string): void {
  void invoke('plugin:log|log', { level, message }).catch(() => {
    // Diagnostics only - never surface a logging failure to the caller.
  });
}

/** Log at INFO to the Rust log. */
export function appLog(message: string): void {
  send(LOG_LEVEL_INFO, message);
}

/** Log at WARN to the Rust log. */
export function appWarn(message: string): void {
  send(LOG_LEVEL_WARN, message);
}
