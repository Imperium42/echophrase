// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Tiny markdown-lite parser.
 * Only supports inline links: [text](url).
 * Returns ordered segments for safe rendering (no innerHTML).
 */

export interface TextSegment {
  type: 'text';
  value: string;
}

export interface LinkSegment {
  type: 'link';
  value: string;
  href: string;
}

export type Segment = TextSegment | LinkSegment;

const LINK_RE = /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g;

/**
 * Parse a single line into text/link segments.
 * Unmatched input is preserved verbatim as text.
 */
export function parseInline(line: string): Segment[] {
  const segments: Segment[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  LINK_RE.lastIndex = 0;
  while ((match = LINK_RE.exec(line)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ type: 'text', value: line.slice(lastIndex, match.index) });
    }
    segments.push({ type: 'link', value: match[1], href: match[2] });
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < line.length) {
    segments.push({ type: 'text', value: line.slice(lastIndex) });
  }

  return segments;
}

/**
 * Split body into lines and parse each.
 */
export function parseLines(body: string): Segment[][] {
  return body.split('\n').map(parseInline);
}
