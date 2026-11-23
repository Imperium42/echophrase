// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import type { TauriSettings } from '$lib/composables/useTauriSettings';

/** What `deliverOutput` should actually do for the configured output modes. */
export interface OutputPlan {
  /** Modes to run, in the user's order. */
  run: string[];
  /**
   * The transcript ends up on the clipboard through paste: copy was requested
   * together with paste, or paste runs without clipboard restore (Settings shows
   * Copy locked on in that case). No separate copy step runs; once paste
   * succeeds the copy is reported (action + toast) as done.
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
 *   If that leaves nothing to run, the transcript is copied to the clipboard
 *   instead so the phrase is never silently dropped.
 */
export function planOutputModes(
  modes: string[],
  settings: Pick<TauriSettings, 'restore_clipboard_after_paste' | 'save_directory'> | null,
  openMic = false
): OutputPlan {
  const paste = modes.includes('paste_at_cursor');
  const copy = modes.includes('copy_to_clipboard');
  const restoreClipboard = paste && !copy && settings?.restore_clipboard_after_paste === true;
  const copyImpliedByPaste = paste && !restoreClipboard;
  let run = modes.filter(mode => {
    if (mode === 'copy_to_clipboard' && copyImpliedByPaste) return false;
    if (mode === 'save_as_file' && openMic && !settings?.save_directory) return false;
    return true;
  });
  if (run.length === 0 && modes.length > 0) run = ['copy_to_clipboard'];
  return { run, copyImpliedByPaste, restoreClipboard };
}
