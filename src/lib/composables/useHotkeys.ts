// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { mainKeyFromEvent } from '$lib/utils/hotkeyKey';
import { writable, derived, get } from 'svelte/store';
import hotkeys from 'hotkeys-js';
import { invoke } from '@tauri-apps/api/core';
import { hotkeyStore } from '$lib/stores/hotkeys';
import { hotkeyAliases } from '$lib/stores/hotkeyAliases';
import { formatHotkey } from '$lib/utils/hotkeyFormatter';
import { translate } from '$lib/locales';
import { platformCache } from '$lib/stores/platformCache';
import { isMac } from '$lib/utils/platform';
import * as globalShortcutService from '$lib/services/globalShortcutService';

// Configuration constants
const HOTKEY_SAVE_COUNTDOWN_SECONDS = 3;
const RECORDING_TIME_UPDATE_INTERVAL_MS = 1000;
const HOTKEY_SHAKE_MS = 600; // red-outline shake duration when an invalid combo is pressed
const HOTKEY_IN_USE_LABEL_MS = 1800; // how long "{combo} is in use" stays on the pill

/**
 * The macOS dedicated Dictation (mic-icon) key. We remap it at the HID layer to
 * F13 (see src-tauri/src/macos_dictation.rs), so the OS and webview see it as
 * F13. We store/register the binding as `f13` but DISPLAY it as "F5" (the label
 * printed on the key). The HID remap is driven by whether this key is bound:
 * bound -> remap on (native dictation disabled); not bound -> remap reverted.
 */
const DICTATION_KEY_TOKEN = 'f13';

/** Best-effort macOS check, preferring the backend-authoritative platform cache. */
function onMac(): boolean {
  if (platformCache.initialized) return platformCache.os.toLowerCase().includes('mac');
  return isMac();
}

/** Enable/disable the macOS dictation-key -> F13 HID remap (no-op off macOS). */
async function setDictationRemap(enabled: boolean): Promise<void> {
  if (!onMac()) return;
  try {
    await invoke('set_dictation_key_remap', { enabled });
  } catch (e) {
    console.warn('[Hotkeys] set_dictation_key_remap failed:', e);
  }
}

/**
 * How the record hotkey behaves in Push to Talk mode (setting `hotkey_mode`):
 * - hybrid (default): tap to start, tap again to stop; OR hold and release to stop
 * - toggle: every press starts/stops; release does nothing
 * - hold: press starts, release always stops
 */
export type HotkeyMode = 'hybrid' | 'toggle' | 'hold';

/** Wiring for press/release handling. Both reuse the one toggle callback. */
export interface HotkeyModeOptions {
  /** Effective mode. Return 'toggle' outside Push to Talk (eg open mic). */
  mode: () => HotkeyMode;
  /** Current recording state (the isRecording store in useRecording.ts). */
  isRecording: () => boolean;
  /**
   * Optional view of the recording composable's start pipeline. Lets the hold
   * handlers (a) treat a press during a slow mic startup as "stop once started"
   * rather than "start again", and (b) only ever stop the session THEY started,
   * never one begun later by a click, the tray or a retry.
   */
  start?: {
    /** True while a dispatched start is still opening the mic. */
    isPending: () => boolean;
    /** Increments on every dispatched start. */
    seq: () => number;
  };
}

/** Hybrid: a press held at least this long counts as hold-to-talk (Wispr Flow / VoiceInk use 0.5s). */
export const HYBRID_HOLD_MS = 500;

// A release that lands before recorder startup flips isRecording true would
// otherwise miss the stop. Poll briefly after release so it still stops.
const RELEASE_POLL_MS = 100;
const RELEASE_POLL_MAX_MS = 3000;

/**
 * Wrap a toggle callback into press/release handlers for the hotkey modes.
 * In hybrid and hold modes OS key-repeat re-fires Pressed while held; those
 * repeats are ignored. In toggle mode every press toggles (repeats included).
 * `reset` clears a stuck held key (eg a keyup lost on the in-window path).
 */
export function createHoldHandlers(
  callback: () => void,
  opts?: HotkeyModeOptions,
  now: () => number = Date.now
): { onPress: () => void; onRelease: () => void; reset: () => void } {
  let held = false; // this press started a recording and the key is still down
  let pressedAt = 0;
  let pollTimer: ReturnType<typeof setTimeout> | null = null;
  /** startSeq of the start this handler dispatched (or is waiting on). */
  let ownedSeq: number | null = null;

  const clearPoll = () => {
    if (pollTimer) clearTimeout(pollTimer);
    pollTimer = null;
  };

  const onPress = () => {
    const mode = opts?.mode() ?? 'toggle';
    if (mode === 'toggle') {
      callback();
      return;
    }
    if (held) return; // key repeat
    const start = opts!.start;
    if (start?.isPending()) {
      // A start is still opening the mic. Pressing now means "stop once it is
      // up", never "start again" (that would open the mic twice).
      ownedSeq = start.seq();
      stopWhenRecording();
      return;
    }
    if (opts!.isRecording()) {
      // hybrid: a tap while a tap-started recording runs stops it.
      // hold: recording without our press (eg started by click) - stop it too.
      callback();
      return;
    }
    held = true;
    pressedAt = now();
    clearPoll();
    const seqBefore = start?.seq();
    callback();
    // Remember which start (if any) this press caused.
    ownedSeq = start && start.seq() !== seqBefore ? start.seq() : null;
  };

  /** Is the start this handler owns still the latest one? False once superseded or never dispatched. */
  const ownsLatestStart = () => {
    const start = opts!.start;
    return !start || (ownedSeq !== null && start.seq() === ownedSeq);
  };

  const stopWhenRecording = () => {
    clearPoll();
    if (!ownsLatestStart()) return; // nothing of ours to stop
    if (opts!.isRecording()) {
      callback();
      return;
    }
    const deadline = now() + RELEASE_POLL_MAX_MS;
    const poll = () => {
      pollTimer = null;
      if (held) return; // pressed again - the new press owns the stop
      if (!ownsLatestStart()) return; // a newer start (click/tray/retry) took over
      if (opts!.isRecording()) callback();
      else if (opts!.start && !opts!.start.isPending())
        return; // our start failed
      else if (now() < deadline) pollTimer = setTimeout(poll, RELEASE_POLL_MS);
    };
    pollTimer = setTimeout(poll, RELEASE_POLL_MS);
  };

  const onRelease = () => {
    if (!opts || !held) return;
    held = false;
    const mode = opts.mode();
    if (mode === 'hold' || (mode === 'hybrid' && now() - pressedAt >= HYBRID_HOLD_MS)) {
      stopWhenRecording();
    }
    // hybrid short tap: keep recording until the next tap
  };

  /** Treat a lost keyup (window blur, tab hidden) as a release so `held` can't stay stuck. */
  const reset = () => onRelease();

  return { onPress, onRelease, reset };
}

/** Does this keyboard event correspond to the main (last) key of a hotkey string? */
export function isMainKeyEvent(
  event: Pick<KeyboardEvent, 'key' | 'code'>,
  hotkey: string
): boolean {
  const main = hotkey.split('+').pop()?.trim().toLowerCase();
  if (!main) return false;
  const key = event.key === ' ' ? 'space' : event.key.toLowerCase();
  if (key === main) return true;
  const code = (event.code ?? '').replace(/^Key|^Digit/, '').toLowerCase();
  return code === main;
}

/** Modifier tokens that can't stand alone as a hotkey. */
const MODIFIER_TOKENS = new Set([
  'ctrl',
  'control',
  'shift',
  'alt',
  'option',
  'meta',
  'cmd',
  'command',
  'super',
]);

// ---------------------------------------------------------------------------
// Shared hotkey recorder (module level, one per app)
//
// useHotkeys() is called more than once (useAppInit registers the global
// hotkey, BottomLeftActions and Settings > Hotkeys drive the recorder). The
// global hotkey handler feeds a re-picked combo into the recorder, so the
// countdown, shake and "in use" state must be shared, not per instance.
// ---------------------------------------------------------------------------

// Reactive state
const countdown = writable(HOTKEY_SAVE_COUNTDOWN_SECONDS);
let countdownInterval: NodeJS.Timeout | null = null;

// Pulses true for a moment to drive a red-outline shake when the user presses
// an invalid combo (e.g. a key with no modifier, or modifiers only).
const hotkeyShake = writable(false);
let shakeTimeout: NodeJS.Timeout | null = null;
function triggerShake() {
  if (shakeTimeout) clearTimeout(shakeTimeout);
  hotkeyShake.set(true);
  shakeTimeout = setTimeout(() => hotkeyShake.set(false), HOTKEY_SHAKE_MS);
}

/** Why a pressed combo was refused (shown for a moment, then cleared). */
export type HotkeyRejection = 'inUse' | 'reservedCopy' | 'reservedPaste' | 'reservedSelectAll';

/**
 * Combos the recorder refuses outright: grabbing them globally would break
 * copy/paste/select-all everywhere (paste-at-cursor itself simulates Ctrl+V).
 * Exact matches only - Ctrl+Shift+C etc. stay allowed. `meta` = Cmd on macOS.
 */
const RESERVED_COMBOS: Readonly<Record<string, HotkeyRejection>> = {
  'ctrl+c': 'reservedCopy',
  'meta+c': 'reservedCopy',
  'ctrl+v': 'reservedPaste',
  'meta+v': 'reservedPaste',
  'ctrl+a': 'reservedSelectAll',
  'meta+a': 'reservedSelectAll',
};

/** The reason `combo` is reserved, or null when it isn't. */
export function reservedComboReason(combo: string): HotkeyRejection | null {
  return RESERVED_COMBOS[combo.toLowerCase()] ?? null;
}

// A refused combo (in use by another app, or reserved): shake, and name it with
// the reason for a moment so the "Nope" is explained instead of looking unresponsive.
const rejectedHotkey = writable<{ combo: string; reason: HotkeyRejection } | null>(null);
let rejectedTimeout: NodeJS.Timeout | null = null;
function rejectCombo(combo: string, reason: HotkeyRejection) {
  if (rejectedTimeout) clearTimeout(rejectedTimeout);
  rejectedHotkey.set({ combo, reason });
  rejectedTimeout = setTimeout(() => rejectedHotkey.set(null), HOTKEY_IN_USE_LABEL_MS);
  triggerShake();
}

// Derived state from hotkey store
const currentHotkey = derived(hotkeyStore, $hotkeyStore => $hotkeyStore.currentHotkey);
const isRecordingHotkey = derived(hotkeyStore, $hotkeyStore => $hotkeyStore.isRecording);
const recordedKeys = derived(hotkeyStore, $hotkeyStore => $hotkeyStore.recordedKeys);

// Display hotkey logic
// hotkeyAliases is a dependency (not just read inside formatHotkey, which uses
// a non-reactive snapshot) so renaming a key refreshes this label immediately.
const displayHotkey = derived(
  [isRecordingHotkey, recordedKeys, currentHotkey, hotkeyAliases],
  ([$isRecordingHotkey, $recordedKeys, $currentHotkey, $hotkeyAliases]) => {
    // recordedKeys holds the full combo as one string (e.g. ["ctrl+alt+space"]).
    // Route both it and the saved hotkey through formatHotkey for platform glyphs.
    const options = { aliases: $hotkeyAliases };
    return $isRecordingHotkey
      ? $recordedKeys.length > 0
        ? formatHotkey($recordedKeys.join('+'), options)
        : translate('hotkeys.pressKeys')
      : formatHotkey($currentHotkey, options);
  }
);

/** The saved hotkey, formatted - shown as "Current: ..." while recording a new one. */
const currentHotkeyLabel = derived(
  [currentHotkey, hotkeyAliases],
  ([$currentHotkey, $hotkeyAliases]) => formatHotkey($currentHotkey, { aliases: $hotkeyAliases })
);

/** The combo just refused (formatted) and why, or null when none. */
const rejection = derived([rejectedHotkey, hotkeyAliases], ([$rejectedHotkey, $hotkeyAliases]) =>
  $rejectedHotkey
    ? {
        label: formatHotkey($rejectedHotkey.combo, { aliases: $hotkeyAliases }),
        reason: $rejectedHotkey.reason,
      }
    : null
);

// WSLg's compositor swallows the Space of Ctrl+Shift+Space (KNOWN_ISSUES.md), so
// that attempt looks like Ctrl+Shift held and released with no third key. Track
// exactly that during a pick so Settings can explain it only when it happens.
const modifierChordWithoutKey = writable(false);
let chordCtrlShift = false;
let chordGotKey = false;

function handleRecorderKeyUp(event: KeyboardEvent) {
  if (event.ctrlKey || event.shiftKey) return; // still holding part of the chord
  if (chordCtrlShift && !chordGotKey) modifierChordWithoutKey.set(true);
  chordCtrlShift = false;
  chordGotKey = false;
}

/**
 * Handle keyboard events during hotkey recording
 */
function handleHotkeyRecording(event: KeyboardEvent) {
  const $isRecordingHotkey = get(isRecordingHotkey) || false;
  if (!$isRecordingHotkey) return;

  event.preventDefault();
  event.stopPropagation();

  // Handle Escape to cancel
  if (event.key === 'Escape') {
    cancelHotkeyRecording();
    return;
  }

  // Only record when a non-modifier key is pressed (the "main" key)
  // Spacebar -> "space" (a literal ' ' once broke Ctrl+Opt+Space), F1-F24
  // from the physical key (Linux renames F13+), unidentified -> event.code.
  let key = mainKeyFromEvent(event);

  if (['control', 'shift', 'alt', 'meta'].includes(key)) {
    if (event.ctrlKey && event.shiftKey) chordCtrlShift = true;
    return; // Don't record just modifier keys
  }
  chordGotKey = true;

  // macOS: the dedicated Dictation (mic) key is remapped to F13 at the HID
  // layer while recording, so it arrives as 'f13'. A plain F5 (fn+F5, or a
  // keyboard without the mic key) also represents the same intent. Normalize
  // both to our canonical 'f13' token, which is displayed as "F5".
  if (onMac() && (key === 'f5' || key === 'f13')) {
    key = DICTATION_KEY_TOKEN;
  }

  // Build the complete combination including all active modifiers
  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push('ctrl');
  if (event.shiftKey) modifiers.push('shift');
  if (event.altKey) modifiers.push('alt');
  if (event.metaKey) modifiers.push('meta');

  // Reject impossible hotkeys: a global shortcut normally needs at least one
  // modifier plus a real (non-modifier) main key. A bare letter/digit would
  // clobber normal typing and most OSes refuse to register it.
  //
  // EXCEPTION: bare function keys (F1-F24) are fine - they don't collide with
  // text input, and the OS happily registers them globally. This is what lets
  // the user bind the dedicated Dictation/mic key (which we remap to F13) or a
  // plain F5 with no modifier. Shake for anything else bare.
  const isFunctionKey = /^f([1-9]|1[0-9]|2[0-4])$/.test(key);
  if ((modifiers.length === 0 && !isFunctionKey) || MODIFIER_TOKENS.has(key)) {
    triggerShake();
    return;
  }

  // Add the main key
  modifiers.push(key);

  void acceptRecordedCombo(modifiers.join('+'));
}

// Guards against an older availability check finishing after a newer press.
let comboCheckId = 0;

/**
 * Record `combo` and start the save countdown, unless another app already
 * holds it - then shake "Nope" and keep listening.
 */
async function acceptRecordedCombo(combo: string) {
  const checkId = ++comboCheckId;
  const reserved = reservedComboReason(combo);
  if (reserved) {
    rejectCombo(combo, reserved);
    return;
  }
  const available = await globalShortcutService.isShortcutAvailable(combo);
  if (checkId !== comboCheckId || !get(isRecordingHotkey)) return;
  if (!available) {
    rejectCombo(combo, 'inUse');
    return;
  }
  modifierChordWithoutKey.set(false);
  rejectedHotkey.set(null);
  hotkeyStore.addRecordedKey(combo);
  startCountdown();
}

/**
 * Start countdown timer for saving hotkey
 */
function startCountdown() {
  // Clear any existing countdown
  if (countdownInterval) {
    clearInterval(countdownInterval);
  }

  countdown.set(HOTKEY_SAVE_COUNTDOWN_SECONDS);
  countdownInterval = setInterval(() => {
    countdown.update(count => {
      const newCount = count - 1;
      if (newCount <= 0) {
        stopHotkeyRecording();
      }
      return newCount;
    });
  }, RECORDING_TIME_UPDATE_INTERVAL_MS);
}

/**
 * Start recording new hotkey
 */
// While picking a new hotkey our own global hotkey is released: Windows and
// macOS hand a registered combo to its owner instead of the window, so
// re-picking the current hotkey would never reach the recorder. setupHotkey
// awaits this before re-registering, so a fast cancel can't be undone by a
// late release. (Ctrl+Shift+Space on WSLg is a different problem: the WSLg
// compositor reserves it - see KNOWN_ISSUES.md.)
let globalSuspended: Promise<void> = Promise.resolve();

/** Any click while the recorder still shows "Press keys..." cancels the pick. */
function handleRecorderPointerDown() {
  if (get(recordedKeys).length === 0) cancelHotkeyRecording();
}

/** Leaving the app (window loses focus) cancels the pick. */
function handleRecorderBlur() {
  cancelHotkeyRecording();
}

function removeRecorderListeners() {
  window.removeEventListener('keydown', handleHotkeyRecording, { capture: true });
  window.removeEventListener('pointerdown', handleRecorderPointerDown, { capture: true });
  window.removeEventListener('blur', handleRecorderBlur);
  window.removeEventListener('keyup', handleRecorderKeyUp, { capture: true });
}

function startHotkeyRecording() {
  rejectedHotkey.set(null);
  modifierChordWithoutKey.set(false);
  chordCtrlShift = false;
  chordGotKey = false;
  hotkeyStore.startRecording();
  countdown.set(HOTKEY_SAVE_COUNTDOWN_SECONDS);
  globalSuspended = globalShortcutService.unregisterCurrent();

  // macOS: enable the dictation-key -> F13 remap for the duration of recording
  // so pressing the mic key is capturable (as F13) instead of launching macOS
  // Dictation. setupHotkey reconciles the remap to the final binding when
  // recording stops/cancels (off again unless F13 was actually chosen).
  void setDictationRemap(true);

  // Add event listeners for recording. The click that started recording has
  // already fired, so only later clicks reach the pointerdown listener.
  window.addEventListener('keydown', handleHotkeyRecording, { capture: true });
  window.addEventListener('pointerdown', handleRecorderPointerDown, { capture: true });
  window.addEventListener('blur', handleRecorderBlur);
  window.addEventListener('keyup', handleRecorderKeyUp, { capture: true });
}

/**
 * Stop hotkey recording and save
 */
function stopHotkeyRecording() {
  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }
  removeRecorderListeners();
  hotkeyStore.stopRecording();
}

/**
 * Cancel hotkey recording without saving
 */
function cancelHotkeyRecording() {
  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }
  removeRecorderListeners();
  hotkeyStore.cancelRecording();
}

/** Clear the recorder timers and its keydown listener (used by cleanup). */
function stopRecorderTimers() {
  if (rejectedTimeout) clearTimeout(rejectedTimeout);
  if (shakeTimeout) clearTimeout(shakeTimeout);
  if (countdownInterval) {
    clearInterval(countdownInterval);
    countdownInterval = null;
  }
  removeRecorderListeners();
}

/**
 * Modern composable for hotkey management
 * Handles global hotkey registration, recording new hotkeys, and countdown logic
 */
export function useHotkeys() {
  // Teardown for the in-window release listeners installed by setupHotkey.
  let releaseListenersCleanup: (() => void) | null = null;
  function removeReleaseListeners() {
    releaseListenersCleanup?.();
    releaseListenersCleanup = null;
  }

  /**
   * Set up global hotkey listener (both browser-level and OS-level)
   */
  async function setupHotkey(
    hotkey: string,
    callback: () => void,
    shouldDisableCallback?: () => boolean,
    hold?: HotkeyModeOptions
  ) {
    const { onPress, onRelease, reset } = createHoldHandlers(callback, hold);

    // In-window fallback only (global registration failed, eg WSL): hotkeys-js
    // misses the keyup when a modifier is let go first or focus leaves the
    // window, which would leave a hold stuck. Release on the main key's own
    // keyup, window blur and tab hide. Replaced on every re-registration.
    removeReleaseListeners();
    const onMainKeyUp = (event: KeyboardEvent) => {
      if (globalShortcutService.hasActiveGlobalShortcut()) return;
      if (isMainKeyEvent(event, hotkey)) onRelease();
    };
    const onFocusLost = () => {
      if (globalShortcutService.hasActiveGlobalShortcut()) return;
      reset();
    };
    const onVisibility = () => {
      if (document.visibilityState === 'hidden') onFocusLost();
    };
    window.addEventListener('keyup', onMainKeyUp, true);
    window.addEventListener('blur', onFocusLost);
    document.addEventListener('visibilitychange', onVisibility);
    releaseListenersCleanup = () => {
      window.removeEventListener('keyup', onMainKeyUp, true);
      window.removeEventListener('blur', onFocusLost);
      document.removeEventListener('visibilitychange', onVisibility);
    };

    // macOS: keep the dictation-key HID remap in sync with the binding. If the
    // mic key (F13) is what's bound, disable native dictation by remapping;
    // otherwise revert so the system Dictation key works normally again.
    void setDictationRemap(hotkey === DICTATION_KEY_TOKEN);

    // Browser-level hotkey (works when app is focused).
    // This used to fire unconditionally, so a single keypress with the window
    // focused would trigger BOTH this handler and the OS-level global
    // shortcut below - double dispatch. The OS-level shortcut is now the
    // single source of truth: this handler only acts when global
    // registration failed to register (see the WSLg note at the top of
    // keyboardService.ts), in which case it's the sole fallback.
    hotkeys.unbind(); // Clear all previous hotkeys
    hotkeys(hotkey, { keyup: true }, event => {
      event.preventDefault();
      if (globalShortcutService.hasActiveGlobalShortcut()) {
        // Global shortcut owns this keypress - stay silent to avoid double dispatch.
        return;
      }
      if (event.type === 'keyup') {
        onRelease();
        return;
      }
      const $isRecordingHotkey = get(isRecordingHotkey) || false;
      const shouldDisable = shouldDisableCallback?.() || false;
      if (!$isRecordingHotkey && !shouldDisable) {
        onPress();
      }
    });

    // OS-level global shortcut (works when app is not focused, and is the
    // single source of truth when it registers successfully).
    const shouldDisableGlobal = () => {
      const $isRecordingHotkey = get(isRecordingHotkey) || false;
      const shouldDisable = shouldDisableCallback?.() || false;
      return $isRecordingHotkey || shouldDisable;
    };

    // Wait for a release from startHotkeyRecording to finish first (see globalSuspended).
    await globalSuspended;
    await globalShortcutService.registerShortcut(hotkey, onPress, shouldDisableGlobal, onRelease);
  }

  /**
   * Initialize hotkey system
   */
  async function initializeHotkeys(
    callback: () => void,
    shouldDisableCallback?: () => boolean,
    hold?: HotkeyModeOptions
  ) {
    // Initialize hotkey store (loads from Tauri settings). Aliases load in the
    // same step so the very first label render already uses the user's names.
    await Promise.all([hotkeyStore.init(), hotkeyAliases.init()]);

    // Initialize global shortcut service FIRST (must complete before registering shortcuts)
    const initSuccess = await globalShortcutService.init();
    if (initSuccess) {
      console.log('[Hotkeys] Global shortcut service ready');
    } else {
      console.warn('[Hotkeys] Global shortcut service failed to initialize');
    }

    // Set up initial hotkey and reactively update when it changes
    const unsubscribe = hotkeyStore.subscribe(state => {
      if (!state.isRecording) {
        setupHotkey(state.currentHotkey, callback, shouldDisableCallback, hold);
      }
    });

    return unsubscribe;
  }

  /**
   * Cleanup function
   */
  async function cleanup() {
    stopRecorderTimers();
    hotkeys.unbind();
    removeReleaseListeners();

    // Cleanup global shortcuts
    await globalShortcutService.cleanup();
  }

  return {
    // State
    currentHotkey: { subscribe: currentHotkey.subscribe },
    isRecordingHotkey: { subscribe: isRecordingHotkey.subscribe },
    recordedKeys: { subscribe: recordedKeys.subscribe },
    displayHotkey: { subscribe: displayHotkey.subscribe },
    countdown: { subscribe: countdown.subscribe },
    hotkeyShake: { subscribe: hotkeyShake.subscribe },
    currentHotkeyLabel: { subscribe: currentHotkeyLabel.subscribe },
    rejection: { subscribe: rejection.subscribe },
    modifierChordWithoutKey: { subscribe: modifierChordWithoutKey.subscribe },

    // Actions
    initializeHotkeys,
    setupHotkey,
    startHotkeyRecording,
    stopHotkeyRecording,
    cancelHotkeyRecording,
    cleanup,
  };
}
