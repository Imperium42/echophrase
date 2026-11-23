// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import type { TauriSettings } from '$lib/composables/useTauriSettings';

/** What `deliverOutput` should actually do for the configured output modes. */
export interface OutputPlan {
  /** Modes to run, in the user's order. */
  run: string[];
  /**
   * Copy was requested together with paste. Paste already puts the transcript
   * on the clipboard, so no separate copy step runs; once paste succeeds the
   * copy is reported (action + toast) as done.
   */
  copyImpliedByPaste: boolean;
  /** Put the previous clipboard text back after pasting. */
  restoreClipboard: boolean;
}

/**
 * Decide which configured output modes actually run.
 *
 * - Copy and paste are independent. Paste goes through the clipboard itself:
 *   - paste only: the transcript stays on the clipboard, or (opt-in
 *     `restore_clipboard_after_paste`) the previous clipboard text comes back;
 *   - paste + copy: the user wants the text on the clipboard, so restore is
 *     ignored and the separate copy step is folded into the paste.
 * - Open mic sends every VAD chunk through the modes; save-as-file would open a
 *   modal save dialog per phrase, so it only runs with an auto-save directory.
 */
export function planOutputModes(
  modes: string[],
  settings: Pick<TauriSettings, 'restore_clipboard_after_paste' | 'save_directory'> | null,
  openMic = false
): OutputPlan {
  const paste = modes.includes('paste_at_cursor');
  const copy = modes.includes('copy_to_clipboard');
  const copyImpliedByPaste = paste && copy;
  const restoreClipboard = paste && !copy && settings?.restore_clipboard_after_paste === true;
  const run = modes.filter(mode => {
    if (mode === 'copy_to_clipboard' && copyImpliedByPaste) return false;
    if (mode === 'save_as_file' && openMic && !settings?.save_directory) return false;
    return true;
  });
  return { run, copyImpliedByPaste, restoreClipboard };
}
