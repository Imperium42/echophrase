// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import type { TauriSettings } from '$lib/composables/useTauriSettings';

/**
 * Decide which configured output modes actually run.
 *
 * - Paste at cursor goes through the clipboard itself. When it is combined with
 *   "restore clipboard after paste", a separate copy step would either make the
 *   paste read the transcript as the "previous" clipboard (copy first) or get
 *   overwritten by the restore (paste first). So copy is folded into paste: paste
 *   puts the text on the clipboard and then restores the previous content.
 * - Open mic sends every VAD chunk through the modes; save-as-file would open a
 *   modal save dialog per phrase, so it only runs with an auto-save directory.
 */
export function planOutputModes(
  modes: string[],
  settings: Pick<TauriSettings, 'restore_clipboard_after_paste' | 'save_directory'> | null,
  openMic = false
): string[] {
  const foldCopyIntoPaste =
    settings?.restore_clipboard_after_paste === true && modes.includes('paste_at_cursor');
  return modes.filter(mode => {
    if (mode === 'copy_to_clipboard' && foldCopyIntoPaste) return false;
    if (mode === 'save_as_file' && openMic && !settings?.save_directory) return false;
    return true;
  });
}
