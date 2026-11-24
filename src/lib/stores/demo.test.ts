// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, beforeEach } from 'vitest';

const platform = vi.hoisted(() => ({ isMac: vi.fn(() => false) }));
vi.mock('$lib/utils/platform', () => platform);

import { demoProvider } from '$lib/stores/demo';

describe('demoProvider', () => {
  beforeEach(() => platform.isMac.mockReset());

  it('shows CoreML on macOS, never the Windows-only DirectML', () => {
    platform.isMac.mockReturnValue(true);
    expect(demoProvider()).toBe('core_ml');
  });

  it('shows DirectML elsewhere (Windows recordings)', () => {
    platform.isMac.mockReturnValue(false);
    expect(demoProvider()).toBe('direct_ml');
  });
});
