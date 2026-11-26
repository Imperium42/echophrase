// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, vi, afterEach } from 'vitest';
import { get } from 'svelte/store';
import { modalController } from './modalController';

function pressEscape() {
  document.body.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
}

describe('modalController ESC (closes only the topmost modal)', () => {
  const opened: string[] = [];
  function open(onClose: () => void, allowEsc = true): string {
    const id = modalController.push({ onClose, allowEsc });
    opened.push(id);
    return id;
  }
  afterEach(() => opened.splice(0).forEach(id => modalController.pop(id)));

  it('a dialog opened from Settings closes first, and Settings stays open', () => {
    const closeSettings = vi.fn();
    const closeSetup = vi.fn();
    open(closeSettings); // Settings
    open(closeSetup); // eg the Transform model download dialog, opened from Settings
    pressEscape();
    expect(closeSetup).toHaveBeenCalledTimes(1);
    expect(closeSettings).not.toHaveBeenCalled();
  });

  it('on a z-index tie, the most recently opened modal is on top', () => {
    const x = open(vi.fn()); // z 200
    const closeY = vi.fn();
    open(closeY); // z 210
    modalController.pop(x);
    const closeZ = vi.fn();
    const z = open(closeZ); // one layer left in the tier -> z 210, same as Y
    expect(get(modalController.topModalId)).toBe(z);
    pressEscape();
    expect(closeZ).toHaveBeenCalledTimes(1);
    expect(closeY).not.toHaveBeenCalled();
  });

  it('skips a top modal that refuses ESC and closes the next one down', () => {
    const closeSettings = vi.fn();
    open(closeSettings);
    open(vi.fn(), false); // eg a modal with no onClose
    pressEscape();
    expect(closeSettings).toHaveBeenCalledTimes(1);
  });
});
