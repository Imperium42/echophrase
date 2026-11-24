// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Development-only logging utility.
 * Logs are stripped in production builds via import.meta.env.DEV check.
 *
 * Use for sensitive debug info that shouldn't appear in production console.
 *
 * @example
 * import { devLog, devWarn, devError } from '$lib/utils/devLog';
 * devLog('[Auth]', 'User logged in:', user.email);
 * devWarn('[License]', 'Activation response:', response);
 */

const isDev = import.meta.env.DEV;

/** localStorage key that re-enables console.log/debug in release builds. */
export const VERBOSE_LOGS_KEY = 'echophrase.verboseLogs';

/** Whether the user opted into verbose console output (release builds only). */
export function isVerboseLoggingEnabled(): boolean {
  try {
    return localStorage.getItem(VERBOSE_LOGS_KEY) === 'true';
  } catch {
    return false;
  }
}

/** Persist the verbose-logging preference and apply it immediately. */
export function setVerboseLoggingEnabled(enabled: boolean): void {
  try {
    if (enabled) localStorage.setItem(VERBOSE_LOGS_KEY, 'true');
    else localStorage.removeItem(VERBOSE_LOGS_KEY);
  } catch {
    // localStorage unavailable - the gate falls back to the build default
  }
  applyConsoleGate();
}

const noop = (): void => {};
const originalLog = console.log;
const originalDebug = console.debug;

/**
 * Silence console.log/console.debug in release builds unless verbose logging is
 * on. console.info/warn/error are never touched so beta users can still report
 * real problems from DevTools. Safe to call repeatedly.
 */
export function applyConsoleGate(): void {
  const verbose = isDev || isVerboseLoggingEnabled();
  console.log = verbose ? originalLog : noop;
  console.debug = verbose ? originalDebug : noop;
}

/**
 * Development-only console.log. No-op in production.
 */
export function devLog(...args: unknown[]): void {
  if (isDev) {
    console.log(...args);
  }
}

/**
 * Development-only console.warn. No-op in production.
 */
export function devWarn(...args: unknown[]): void {
  if (isDev) {
    console.warn(...args);
  }
}

/**
 * Development-only console.error. No-op in production.
 * Note: For actual errors that should always be logged, use console.error directly.
 */
export function devError(...args: unknown[]): void {
  if (isDev) {
    console.error(...args);
  }
}

/**
 * Development-only console.debug. No-op in production.
 */
export function devDebug(...args: unknown[]): void {
  if (isDev) {
    console.debug(...args);
  }
}
