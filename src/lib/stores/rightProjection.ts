// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writable } from 'svelte/store';

/** Which left-chip is currently hovered, driving right-side content. */
export interface RightProjection {
  /** Label to show as the bubble header (e.g. "Raw Transcript", "Transformed"). */
  label: string;
  /** Body text to display and copy on click. */
  text: string;
}

export const rightProjection = writable<RightProjection | null>(null);
