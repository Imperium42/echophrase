// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { planOutputModes } from './outputPlan';

describe('planOutputModes', () => {
  const both = ['copy_to_clipboard', 'paste_at_cursor'];
  const bothReversed = ['paste_at_cursor', 'copy_to_clipboard'];

  it.each([[both], [bothReversed]])('folds copy into paste when restore is on (%j)', modes => {
    expect(planOutputModes(modes, { restore_clipboard_after_paste: true })).toEqual([
      'paste_at_cursor',
    ]);
  });

  it('keeps both modes when restore is off', () => {
    expect(planOutputModes(both, { restore_clipboard_after_paste: false })).toEqual(both);
  });

  it('keeps copy when there is no paste, even with restore on', () => {
    expect(planOutputModes(['copy_to_clipboard'], { restore_clipboard_after_paste: true })).toEqual(
      ['copy_to_clipboard']
    );
  });

  it('skips save_as_file for open mic without an auto-save directory', () => {
    const modes = ['copy_to_clipboard', 'save_as_file'];
    expect(planOutputModes(modes, {}, true)).toEqual(['copy_to_clipboard']);
    expect(planOutputModes(modes, { save_directory: '/tmp/x' }, true)).toEqual(modes);
    expect(planOutputModes(modes, {}, false)).toEqual(modes);
  });
});
