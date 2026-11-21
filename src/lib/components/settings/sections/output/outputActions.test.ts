// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect } from 'vitest';
import { highlightJSON } from './outputActions';

describe('highlightJSON', () => {
  it('escapes markup so user templates cannot inject HTML', () => {
    const out = highlightJSON('{"text": "<img src=x onerror=alert(1)>"}');
    expect(out).not.toContain('<img');
    expect(out).toContain('&lt;img src=x onerror=alert(1)&gt;');
  });

  it('still highlights keys, values and template placeholders', () => {
    const out = highlightJSON('{"text": "{{transcript}}", "n": 3}');
    expect(out).toContain('<span class="json-key">"text"</span>');
    expect(out).toContain('<span class="json-number">3</span>');
    expect(out).toContain('<span class="json-template">{<!-- -->{transcript}<!-- -->}</span>');
  });
});
