// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import {
  appendTrailingSpaceIfIncomplete,
  convertSpokenPunctuation,
  fixBrandName,
} from './useTranscriptFormatter';

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

describe('convertSpokenPunctuation', () => {
  it('turns a trailing spoken question mark into a symbol', () => {
    expect(convertSpokenPunctuation('is this working question mark')).toBe('is this working?');
  });

  it('dedupes punctuation the model added around the phrase', () => {
    expect(convertSpokenPunctuation('Is this working? Question mark.')).toBe('Is this working?');
    expect(convertSpokenPunctuation('wow, exclamation mark,')).toBe('wow!');
    expect(convertSpokenPunctuation('Really. Question mark!')).toBe('Really?');
  });

  it('capitalizes the word that follows a converted mark', () => {
    expect(convertSpokenPunctuation("wow exclamation mark that's cool")).toBe("wow! That's cool");
    expect(convertSpokenPunctuation('ready question mark, yes')).toBe('ready? Yes');
  });

  it('accepts "exclamation point" and hyphenated forms', () => {
    expect(convertSpokenPunctuation('stop exclamation point')).toBe('stop!');
    expect(convertSpokenPunctuation('why question-mark')).toBe('why?');
  });

  it('is case-insensitive', () => {
    expect(convertSpokenPunctuation('Done EXCLAMATION MARK')).toBe('Done!');
  });

  it('converts a standalone phrase', () => {
    expect(convertSpokenPunctuation('Question mark.')).toBe('?');
  });

  it('does not touch plurals or unrelated text', () => {
    expect(convertSpokenPunctuation('use two question marks here')).toBe(
      'use two question marks here'
    );
    expect(convertSpokenPunctuation('a questionable mark')).toBe('a questionable mark');
  });

  it('handles empty input', () => {
    expect(convertSpokenPunctuation('')).toBe('');
  });
});

describe('fixBrandName', () => {
  it('joins "Echo phrase" into the app name', () => {
    expect(fixBrandName('I use Echo phrase daily.')).toBe('I use Echophrase daily.');
  });

  it('handles any case, hyphens and the already-joined lowercase form', () => {
    expect(fixBrandName('echo phrase')).toBe('Echophrase');
    expect(fixBrandName('ECHO PHRASE')).toBe('Echophrase');
    expect(fixBrandName('Echo-phrase')).toBe('Echophrase');
    expect(fixBrandName('echophrase rocks')).toBe('Echophrase rocks');
  });

  it('keeps possessives and punctuation around it', () => {
    expect(fixBrandName("Echo phrase's settings, echo phrase!")).toBe(
      "Echophrase's settings, Echophrase!"
    );
  });

  it('leaves the plural and longer words alone', () => {
    expect(fixBrandName('echo phrases')).toBe('echo phrases');
    expect(fixBrandName('echo phrasebook')).toBe('echo phrasebook');
    expect(fixBrandName('the echo of a phrase')).toBe('the echo of a phrase');
  });
});
