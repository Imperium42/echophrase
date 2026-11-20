// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * UI Scale composable (Settings > Accessibility)
 *
 * Applies the effective UI scale as CSS zoom on :root. CSS zoom scales both
 * rem and px sizing uniformly and works in WebKitGTK / WebView2 / WKWebView,
 * unlike the native WebviewWindow::set_zoom which is unreliable on some
 * compositors (notably WSLg).
 *
 * Precedence for the effective scale:
 *   1. scale-with-window ON  -> base x (windowWidth / referenceWidth), clamped
 *   2. mode == "manual"      -> ui_scale_manual
 *   3. mode == "auto"        -> 1.0 (no extra zoom)
 *
 * WHY auto == 1.0: the webview already renders CSS pixels at the display's
 * native resolution - the OS/compositor has ALREADY applied the display scale
 * factor (2.0 on macOS Retina, 1.5 at 150% Windows). Feeding that same factor
 * back in as CSS zoom double-applies it and produces a giant UI + oversized
 * window (the macOS "massive window on first launch" regression). The OS scale
 * factor is device pixel density, NOT a user "make things bigger" preference,
 * so Auto adds nothing on top. Users who want a larger/smaller UI use Manual.
 * Never reintroduce multiplying zoom by window.scale_factor().
 *
 * The reference width for scale-with-window is captured in memory when the
 * toggle turns on (or on init when already on) and is never persisted.
 * The last applied zoom is cached in localStorage and re-applied immediately
 * on startup to minimize flash before settings load.
 */
import { writable } from 'svelte/store';
import { tauriSettings, type TauriSettings } from './useTauriSettings';
import { devWarn } from '$lib/utils/devLog';
import { DEFAULT_WINDOW_SIZE } from '$lib/config/window';

/** Supported UI scale range - keep in sync with Rust UI_SCALE_MIN/MAX */
export const UI_SCALE_MIN = 0.5;
export const UI_SCALE_MAX = 2.0;

/** localStorage key for the startup flash-prevention cache */
const ZOOM_CACHE_KEY = 'echophrase.uiScale';

/** Inputs to the pure effective-scale computation (unit-testable) */
export interface UiScaleInputs {
  mode: 'auto' | 'manual';
  manual: number;
  withWindow: boolean;
  windowWidth: number | null;
  referenceWidth: number | null;
}

/** Clamp a scale factor to the supported 50%-200% range */
export function clampScale(value: number): number {
  if (!Number.isFinite(value) || value <= 0) return 1.0;
  return Math.min(UI_SCALE_MAX, Math.max(UI_SCALE_MIN, value));
}

/**
 * Compute the effective UI scale per the documented precedence.
 * Pure function - all environment inputs are passed in.
 *
 * Auto mode is a constant 1.0 by design (see the module header): the webview
 * already renders at native DPI, so no extra zoom is applied. Manual mode is
 * the user-facing scaling control.
 */
export function computeEffectiveScale(inputs: UiScaleInputs): number {
  const base = inputs.mode === 'manual' ? clampScale(inputs.manual) : 1.0;

  if (inputs.withWindow && inputs.windowWidth && inputs.referenceWidth) {
    return clampScale(base * (inputs.windowWidth / inputs.referenceWidth));
  }
  return base;
}

/** Currently applied effective zoom (for the live % label in Settings) */
export const uiScaleEffective = writable(1.0);

/** Apply a scale factor to <body> and cache it for the next startup */
function applyZoom(zoom: number): void {
  if (typeof document === 'undefined' || !document.body) return;
  const clamped = clampScale(zoom);
  // CSS zoom (standardized 2024) relayouts at native resolution, so text and
  // icons stay crisp. transform: scale() was tried and rejected: WebKitGTK
  // rasterizes the layer at 1x and upscales the bitmap (blurry/aliased UI).
  // The rightward-shift bug once blamed on zoom was actually a webview/window
  // desync from a silently-denied setSize (missing core:window:allow-set-size
  // capability) - fixed in capabilities/default.json.
  document.documentElement.style.setProperty('zoom', clamped === 1 ? '' : String(clamped));
  // Viewport units are NOT multiplied by CSS zoom (100vw/100vh stay physical
  // while other lengths zoom). Styles that mix them with zoomed content must
  // divide the factor out: calc(100vh / var(--ui-zoom, 1)).
  document.documentElement.style.setProperty('--ui-zoom', String(clamped));
  uiScaleEffective.set(clamped);
  try {
    localStorage.setItem(ZOOM_CACHE_KEY, String(clamped));
  } catch {
    // localStorage unavailable (private mode etc.) - cache is best-effort only
  }
}

/**
 * The zoom factor the last window-size check ran for. Used only to skip
 * redundant resize calls; the resize itself targets DEFAULT_WINDOW_SIZE x
 * zoom absolutely (see resizeWindowTo). NOT updated by the startup
 * localStorage flash-apply, which changes CSS zoom but not the window.
 */
let windowZoom = 1.0;

/** Serializes window resizes so overlapping slider changes cannot interleave */
let resizeChain: Promise<void> = Promise.resolve();

/**
 * Ensure the window meets the per-zoom minimum size of DEFAULT_WINDOW_SIZE x
 * zoom (queued, Tauri only). The minimum is ABSOLUTE - derived from the
 * DEFAULT window size, never the current one (relative resizes compound when
 * calls overlap in flight). Grow-only: a window already at or above the
 * minimum is left alone, scales <= 100% enforce nothing, and lowering the
 * zoom never shrinks the window (the Accessibility Reset button restores the
 * default size). Note: major apps rescale content only and never resize the
 * window on zoom change - this minimum-size nudge is a deliberate exception
 * so scaled-up UI always has room to fit.
 */
function resizeWindowTo(zoom: number): void {
  if (zoom <= 1) return;
  resizeChain = resizeChain.then(async () => {
    try {
      const { getCurrentWindow, LogicalSize } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      const inner = await win.innerSize();
      const sf = await win.scaleFactor();
      const current = inner.toLogical(sf);
      const target = new LogicalSize(
        Math.max(current.width, DEFAULT_WINDOW_SIZE.width * zoom),
        Math.max(current.height, DEFAULT_WINDOW_SIZE.height * zoom)
      );
      if (target.width - current.width < 1 && target.height - current.height < 1) return;
      await win.setSize(target);
      // Some WMs (notably WSLg) clamp or ignore programmatic resizes, which
      // can leave the webview laid out for a size the window never reached.
      // Nudge a relayout at whatever size actually stuck.
      await new Promise(resolve => setTimeout(resolve, 50));
      window.dispatchEvent(new Event('resize'));
    } catch (e) {
      devWarn('[useUiScale] window resize failed', e);
    }
  });
}

/**
 * Runtime state - module-level singleton so the composable can be called
 * from anywhere without double-wiring listeners.
 */
let initialized = false;
let referenceWidth: number | null = null;
let lastWithWindow = false;
let unsubscribeSettings: (() => void) | null = null;
let unlistenResize: (() => void) | null = null;

function recompute(settings: TauriSettings | null): void {
  const mode = settings?.ui_scale_mode === 'manual' ? 'manual' : 'auto';
  const withWindow = settings?.ui_scale_with_window ?? false;

  // Capture the reference width when scale-with-window flips on
  if (withWindow && !lastWithWindow) {
    referenceWidth = typeof window !== 'undefined' ? window.innerWidth : null;
  }
  if (!withWindow) {
    referenceWidth = null;
  }
  lastWithWindow = withWindow;

  const next = computeEffectiveScale({
    mode,
    manual: settings?.ui_scale_manual ?? 1.0,
    withWindow,
    windowWidth: typeof window !== 'undefined' ? window.innerWidth : null,
    referenceWidth,
  });

  // Keep the window size in lockstep with the zoom (startup included) so
  // wide content keeps fitting at higher scales. Never in with-window mode
  // (the zoom is derived FROM the window size there - feedback loop).
  if (!withWindow && Math.abs(next - windowZoom) > 0.001) {
    resizeWindowTo(next);
    windowZoom = next;
  }

  applyZoom(next);
}

/**
 * Wire up UI scaling once at app root. Applies the cached zoom immediately,
 * then tracks settings changes and window resizes. Returns a cleanup function.
 */
export function initUiScale(): () => void {
  if (initialized || typeof window === 'undefined') {
    return () => {};
  }
  initialized = true;

  // Flash prevention: re-apply the last known zoom before settings load
  try {
    const cached = Number(localStorage.getItem(ZOOM_CACHE_KEY));
    if (Number.isFinite(cached) && cached > 0 && cached !== 1) {
      applyZoom(cached);
    }
  } catch {
    // best-effort only
  }

  // React to settings changes (mode / manual value / with-window toggle)
  unsubscribeSettings = tauriSettings.subscribe(settings => {
    if (settings) recompute(settings);
  });

  // Subscribe to window resizes for scale-with-window mode (Tauri only)
  if (typeof (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ !== 'undefined') {
    void (async () => {
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const win = getCurrentWindow();
        unlistenResize = await win.onResized(() => {
          if (lastWithWindow) {
            let current: TauriSettings | null = null;
            const unsub = tauriSettings.subscribe(s => (current = s));
            unsub();
            recompute(current);
          }
        });
      } catch (e) {
        devWarn('[useUiScale] window event wiring failed', e);
      }
    })();
  }

  return () => {
    initialized = false;
    unsubscribeSettings?.();
    unsubscribeSettings = null;
    unlistenResize?.();
    unlistenResize = null;
  };
}
