// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { planOutputModes } from './outputPlan';

describe('planOutputModes', () => {
  const on = { restore_clipboard_after_paste: true };
  const off = { restore_clipboard_after_paste: false };

  it('paste only + restore on: paste runs and restores', () => {
    expect(planOutputModes(['paste_at_cursor'], on)).toEqual({
      run: ['paste_at_cursor'],
      copyImpliedByPaste: false,
      restoreClipboard: true,
    });
  });

  it('paste only + restore off: transcript stays on the clipboard, reported as copied', () => {
    expect(planOutputModes(['paste_at_cursor'], off)).toEqual({
      run: ['paste_at_cursor'],
      copyImpliedByPaste: true,
      restoreClipboard: false,
    });
  });

  it.each([[['copy_to_clipboard', 'paste_at_cursor']], [['paste_at_cursor', 'copy_to_clipboard']]])(
    'copy + paste (%j): restore ignored, copy folded into paste',
    modes => {
      expect(planOutputModes(modes, on)).toEqual({
        run: ['paste_at_cursor'],
        copyImpliedByPaste: true,
        restoreClipboard: false,
      });
    }
  );

  it('copy only: plain copy, restore irrelevant', () => {
    expect(planOutputModes(['copy_to_clipboard'], on)).toEqual({
      run: ['copy_to_clipboard'],
      copyImpliedByPaste: false,
      restoreClipboard: false,
    });
  });

  it('skips save_as_file for open mic without an auto-save directory', () => {
    const modes = ['copy_to_clipboard', 'save_as_file'];
    expect(planOutputModes(modes, {}, true).run).toEqual(['copy_to_clipboard']);
    expect(planOutputModes(modes, { save_directory: '/tmp/x' }, true).run).toEqual(modes);
    expect(planOutputModes(modes, {}, false).run).toEqual(modes);
  });

  it('open mic with only save_as_file and no auto-save dir falls back to copy', () => {
    expect(planOutputModes(['save_as_file'], {}, true)).toEqual({
      run: ['copy_to_clipboard'],
      copyImpliedByPaste: false,
      restoreClipboard: false,
    });
    expect(planOutputModes(['save_as_file'], { save_directory: '/tmp/x' }, true).run).toEqual([
      'save_as_file',
    ]);
  });
});
