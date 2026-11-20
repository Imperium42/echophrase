// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { appendTrailingSpaceIfIncomplete } from './useTranscriptFormatter';

describe('appendTrailingSpaceIfIncomplete', () => {
  it('appends a space when text lacks sentence-ending punctuation', () => {
    expect(appendTrailingSpaceIfIncomplete('Hello, my name is')).toBe('Hello, my name is ');
    expect(appendTrailingSpaceIfIncomplete('Dylan')).toBe('Dylan ');
  });

  it('does not append when text ends with terminal punctuation', () => {
    expect(appendTrailingSpaceIfIncomplete('Hello.')).toBe('Hello.');
    expect(appendTrailingSpaceIfIncomplete('Really?')).toBe('Really?');
    expect(appendTrailingSpaceIfIncomplete('Stop!')).toBe('Stop!');
  });

  it('normalizes existing trailing whitespace to a single space', () => {
    expect(appendTrailingSpaceIfIncomplete('word   ')).toBe('word ');
  });

  it('leaves empty or whitespace-only text unchanged', () => {
    expect(appendTrailingSpaceIfIncomplete('')).toBe('');
    expect(appendTrailingSpaceIfIncomplete('   ')).toBe('   ');
  });
});
