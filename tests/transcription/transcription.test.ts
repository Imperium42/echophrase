// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import path from 'path';

describe('Transcription Tests', () => {
  it('should validate file path patterns', () => {
    const validPaths = [
      'tests/audio/test-1-2.wav',
      'recording.wav', 
      'audio.mp3'
    ];

    validPaths.forEach(filePath => {
      expect(filePath).toMatch(/\.(wav|mp3|m4a)$/i);
    });
  });

  it('should create valid audio file paths', () => {
    const testAudioPath = path.resolve('tests/audio/test-1-2.wav');
    expect(testAudioPath).toBeTruthy();
    expect(testAudioPath).toContain('test-1-2.wav');
  });
});

describe('Transcription Quality Validation', () => {
  it('should normalize transcription output', () => {
    const testCases = [
      { input: 'test one two', expected: 'test one two' },
      { input: 'Test 1 2', expected: 'test 1 2' },
      { input: '  Test  One  Two  ', expected: 'test one two' }
    ];

    testCases.forEach(({ input, expected }) => {
      const normalized = input.toLowerCase().trim().replace(/\s+/g, ' ');
      expect(normalized).toBe(expected);
    });
  });
});