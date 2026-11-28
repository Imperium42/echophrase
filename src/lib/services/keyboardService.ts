// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Keyboard Service
 *
 * Unified service for:
 * 1. OS-level global shortcuts via Tauri's global-shortcut plugin (works when app is not focused)
 * 2. In-app keyboard shortcuts (ESC modal close, F5 refresh, Ctrl+Shift+I devtools, etc.)
 *
 * Previously split across globalShortcutService.ts (271L) and globalKeyboardService.ts (203L).
 * Merged here because both files register and dispatch keyboard events - the split was artificial.
 *
 * KNOWN ISSUES (global shortcuts):
 * - WSL2/WSLg: Global shortcuts only work within X11/WSLg apps, not native Windows apps.
 *   This is a platform limitation - works correctly in production Windows builds.
 * - If shortcuts register but don't trigger, try: `rm bun.lock node_modules && bun install`
 *   See: https://github.com/tauri-apps/plugins-workspace/issues/2646
 */

import { writable, get } from 'svelte/store';
import { mustInvoke } from '$lib/ipc/invoke';
import { debugHidden } from '$lib/stores/debugVisibility';
import { bannersHidden } from '$lib/stores/bannerVisibility';
import { APP_ENV } from '$lib/config/environment';

// ===========================================================================
// Section 1 - OS-level global shortcut registration (Tauri plugin)
// ===========================================================================

// Types from Tauri plugin
interface ShortcutEvent {
  shortcut: string;
  id: number;
  state: 'Pressed' | 'Released';
}

type ShortcutHandler = (event: ShortcutEvent) => void;

// Plugin API (dynamically imported)
let register: ((shortcuts: string | string[], handler: ShortcutHandler) => Promise<void>) | null =
  null;
let unregister: ((shortcuts: string | string[]) => Promise<void>) | null = null;
let unregisterAll: (() => Promise<void>) | null = null;

// Store
export const isInitialized = writable(false);
export const lastError = writable<string | null>(null);

// Track currently registered shortcut
let currentShortcut: string | null = null;
let currentCallback: (() => void) | null = null;
let currentShouldDisable: (() => boolean) | null = null;
let currentOnRelease: (() => void) | null = null;

/**
 * Convert hotkeys-js format to Tauri accelerator format.
 * Examples:
 *   "ctrl+shift+a" -> "CommandOrControl+Shift+A"
 *   "ctrl+space"   -> "CommandOrControl+Space"
 */
export function convertToAccelerator(hotkey: string): string {
  const parts = hotkey.toLowerCase().split('+');

  const converted = parts.map(part => {
    switch (part) {
      case 'ctrl':
      case 'control':
        return 'CommandOrControl';
      case 'meta':
      case 'cmd':
      case 'command':
        return 'Super';
      case 'alt':
        return 'Alt';
      case 'shift':
        return 'Shift';
      case 'space':
        return 'Space';
      case 'enter':
      case 'return':
        return 'Enter';
      case 'esc':
      case 'escape':
        return 'Escape';
      case 'tab':
        return 'Tab';
      case 'backspace':
        return 'Backspace';
      case 'delete':
        return 'Delete';
      case 'up':
        return 'Up';
      case 'down':
        return 'Down';
      case 'left':
        return 'Left';
      case 'right':
        return 'Right';
      default:
        if (part.length === 1 && /[a-z]/.test(part)) return part.toUpperCase();
        if (/^f\d{1,2}$/.test(part)) return part.toUpperCase();
        return part;
    }
  });

  return converted.join('+');
}

/**
 * Initialize the global shortcut service.
 * Dynamically imports the Tauri plugin and clears any orphaned shortcuts
 * from previous sessions.
 */
export async function init(): Promise<boolean> {
  if (get(isInitialized)) return true;

  try {
    const plugin = await import('@tauri-apps/plugin-global-shortcut');
    register = plugin.register;
    unregister = plugin.unregister;
    unregisterAll = plugin.unregisterAll;

    // Defensive cleanup: clear orphaned shortcuts from crashed/previous sessions
    try {
      await plugin.unregisterAll();
      console.log('[Keyboard] Defensive cleanup: cleared any orphaned shortcuts');
    } catch {
      console.log('[Keyboard] No orphaned shortcuts to clear');
    }

    isInitialized.set(true);
    lastError.set(null);
    console.log('[Keyboard] Global shortcut service initialized');
    return true;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    lastError.set(message);
    console.error('[Keyboard] Failed to initialize global shortcuts:', message);
    return false;
  }
}

/**
 * Register a global OS-level shortcut.
 *
 * @param hotkey - Hotkey in hotkeys-js format (e.g., "ctrl+shift+a")
 * @param callback - Function to call when shortcut is pressed
 * @param shouldDisable - Optional guard that returns true to skip the callback
 * @param onRelease - Optional key-release handler (hold-to-talk). Not gated by
 *   shouldDisable, so a stop can never be swallowed.
 */
export async function registerShortcut(
  hotkey: string,
  callback: () => void,
  shouldDisable?: () => boolean,
  onRelease?: () => void
): Promise<boolean> {
  if (!register || !unregister) {
    console.warn('[Keyboard] Service not initialized');
    return false;
  }

  const accelerator = convertToAccelerator(hotkey);

  // Unregister previous shortcut if different. Clear currentShortcut
  // immediately so hasActiveGlobalShortcut() doesn't report a shortcut that
  // was just torn down as still active while re-registration is in flight
  // (or failed below) - that would wrongly keep the browser-level fallback
  // silent.
  if (currentShortcut && currentShortcut !== accelerator) {
    try {
      await unregister(currentShortcut);
      console.log('[Keyboard] Unregistered previous:', currentShortcut);
    } catch {
      // Ignore - may not be registered
    }
    currentShortcut = null;
  }

  // Skip re-registration when same shortcut is already active
  if (currentShortcut === accelerator) {
    currentCallback = callback;
    currentShouldDisable = shouldDisable ?? null;
    currentOnRelease = onRelease ?? null;
    return true;
  }

  // Retry up to 3x - registration races against OS cleanup of shortcuts from
  // a previous (just-exited) process. A quick retry usually succeeds.
  const MAX_ATTEMPTS = 3;
  const RETRY_DELAY_MS = 1000;
  let lastErr: unknown = null;

  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
    try {
      await register(accelerator, (event: ShortcutEvent) => {
        if (event.state === 'Released') {
          currentOnRelease?.();
          return;
        }
        if (event.state !== 'Pressed') return;
        if (currentShouldDisable?.()) {
          console.log('[Keyboard] Callback disabled');
          return;
        }
        console.log('[Keyboard] Global shortcut triggered:', event.shortcut);
        currentCallback?.();
      });

      currentShortcut = accelerator;
      currentCallback = callback;
      currentShouldDisable = shouldDisable ?? null;
      currentOnRelease = onRelease ?? null;
      lastError.set(null);

      if (attempt > 1) {
        console.log(`[Keyboard] Registered on attempt ${attempt}:`, accelerator);
      } else {
        console.log('[Keyboard] Registered global shortcut:', accelerator);
      }
      return true;
    } catch (error) {
      lastErr = error;
      const message = error instanceof Error ? error.message : String(error);
      if (attempt < MAX_ATTEMPTS) {
        console.warn(
          `[Keyboard] Register attempt ${attempt}/${MAX_ATTEMPTS} failed (${message}) - retrying in ${RETRY_DELAY_MS}ms`
        );
        // Only this accelerator: unregisterAll would also drop shortcuts Rust
        // owns (the global Esc cancel, dev F5 reload).
        try {
          await unregister(accelerator);
        } catch {
          /* ignore */
        }
        await new Promise(resolve => setTimeout(resolve, RETRY_DELAY_MS));
      }
    }
  }

  const finalMessage = lastErr instanceof Error ? lastErr.message : String(lastErr);
  // Dev-only hint stays in the console; the store carries the plain message
  // because HotkeysSection renders it to end users.
  const diagnostic = finalMessage.toLowerCase().includes('already registered')
    ? ' - another Echophrase instance may still be running. Run: pgrep -af target/debug/echophrase'
    : '';
  lastError.set(finalMessage);
  console.error(
    `[Keyboard] Failed to register ${accelerator} after ${MAX_ATTEMPTS} attempts: ${finalMessage}${diagnostic}`
  );
  return false;
}

/**
 * Whether an OS-level global shortcut is currently registered and active.
 * Used by the browser-level (hotkeys-js) handler in useHotkeys.ts to know
 * whether it should stay silent (global shortcut is the single source of
 * truth) or act as a fallback (global registration failed, e.g. WSLg).
 */
export function hasActiveGlobalShortcut(): boolean {
  return currentShortcut !== null;
}

/**
 * Whether `hotkey` (hotkeys-js format) can be registered right now: registers
 * it with a no-op handler, then releases it. Our own active hotkey counts as
 * available (re-picking the same combo). True when the plugin isn't loaded,
 * since there's nothing to check against (the save path reports failures).
 */
export async function isShortcutAvailable(hotkey: string): Promise<boolean> {
  if (!register || !unregister) return true;
  const accelerator = convertToAccelerator(hotkey);
  if (accelerator === currentShortcut) return true;
  try {
    await register(accelerator, () => {});
  } catch {
    return false; // another app (or OS) holds it
  }
  try {
    await unregister(accelerator);
  } catch {
    // Best effort: the next registerShortcut unregisters stale shortcuts anyway
  }
  return true;
}

/**
 * Unregister the currently active global shortcut.
 */
export async function unregisterCurrent(): Promise<void> {
  if (!unregister || !currentShortcut) return;

  try {
    await unregister(currentShortcut);
    console.log('[Keyboard] Unregistered:', currentShortcut);
    currentShortcut = null;
    currentCallback = null;
    currentShouldDisable = null;
    currentOnRelease = null;
  } catch (error) {
    console.error('[Keyboard] Failed to unregister:', error);
  }
}

/**
 * Unregister all global shortcuts and reset state.
 */
export async function cleanup(): Promise<void> {
  if (!unregisterAll) return;

  try {
    await unregisterAll();
    currentShortcut = null;
    currentCallback = null;
    currentShouldDisable = null;
    currentOnRelease = null;
    console.log('[Keyboard] Cleaned up all global shortcuts');
  } catch (error) {
    console.error('[Keyboard] Failed to cleanup:', error);
  }
}

// ===========================================================================
// Section 2 - In-app keyboard shortcuts (document keydown listener)
// ===========================================================================

/** Context object passed to setupGlobalKeyboard from the page component. */
export interface KeyboardContext {
  /** @deprecated - use mustInvoke from $lib/ipc/invoke directly */
  invoke?: unknown;
  /** Check if any modal is open (includes modalController registrations) */
  isAnyModalOpen: () => boolean;
  /** Cancel active recording without transcribing (ESC during recording) */
  cancelRecording: () => void;
  /** State the ESC priority needs (modals themselves are modalController layers) */
  modalStates: {
    /** Whether a recording is currently active (push-to-talk or open mic) */
    isRecording: () => boolean;
  };
}

/**
 * Install in-app keyboard shortcuts on the document.
 * Returns a cleanup function to remove the listener (call from onDestroy).
 *
 * @example
 * ```typescript
 * const cleanup = setupGlobalKeyboard({
 *   isAnyModalOpen: () => showSettings,
 *   closeSettings: () => (showSettings = false),
 *   // ...
 * });
 * onDestroy(cleanup);
 * ```
 */
export function setupGlobalKeyboard(context: KeyboardContext): () => void {
  async function handleKeydown(e: KeyboardEvent) {
    const { cancelRecording, modalStates } = context;

    // ESC - cancel an active recording; modals are closed by modalController
    if (e.key === 'Escape') {
      e.preventDefault();

      // Priority 1: Cancel active recording - takes precedence over all modals
      if (modalStates.isRecording()) {
        console.log('[ESC] Cancelling recording');
        cancelRecording();
        e.stopImmediatePropagation();
        return;
      }

      // Everything else (Settings, the model download dialog, the custom model
      // import window, dropdowns) is a modalController layer: its window
      // listener closes ONLY the topmost one, so a dialog opened from Settings
      // closes before Settings. Don't close modals here.
      return;
    }

    // Ctrl+Shift+I / Cmd+Shift+I - Toggle devtools (dev/staging only)
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'I') {
      if (APP_ENV === 'production') {
        console.log('[Keyboard] Devtools disabled in production');
        return;
      }
      e.preventDefault();
      console.log('[Keyboard] Ctrl+Shift+I - toggling devtools');
      try {
        await mustInvoke('toggle_devtools');
      } catch (err) {
        if (String(err).includes('not found')) {
          console.log('[Keyboard] Devtools not available in this build');
        } else {
          console.error('[Keyboard] Failed to toggle devtools:', err);
        }
      }
      return;
    }

    // F5 - Refresh page
    if (e.key === 'F5') {
      e.preventDefault();
      console.log('[Keyboard] F5 - reloading page');
      window.location.reload();
      return;
    }

    // F4 - Cancel any active hide window (dev only). Restores BOTH the debug
    // panel and the top-left banners, so one keypress always gets you back to a
    // normal screen regardless of which "Hide 5m" button was pressed.
    if (e.key === 'F4' && import.meta.env.DEV) {
      e.preventDefault();
      debugHidden.show();
      bannersHidden.show();
      console.log('[Keyboard] F4 - debug panel + banners restored');
      return;
    }

    // Ctrl+Shift+R / Cmd+Shift+R - Hard refresh (clear cache + reload)
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'R') {
      e.preventDefault();
      console.log('[Keyboard] Ctrl+Shift+R - hard refresh');
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        await getCurrentWebview().clearAllBrowsingData();
        console.log('[Keyboard] Cleared browsing data');
      } catch (err) {
        console.warn('[Keyboard] Could not clear browsing data:', err);
      }
      window.location.reload();
      return;
    }
  }

  document.addEventListener('keydown', handleKeydown);
  console.log('[Keyboard] In-app keyboard shortcuts installed');

  return () => {
    document.removeEventListener('keydown', handleKeydown);
  };
}
