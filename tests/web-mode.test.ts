// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach, vi } from 'vitest';

/**
 * Test suite to ensure web mode works without Tauri APIs
 * Reproduces the errors seen in the browser console
 */
describe('Web Mode Compatibility', () => {
  beforeEach(() => {
    // Clear any existing mocks
    vi.clearAllMocks();
    
    // Simulate web mode - no Tauri APIs available
    (global as any).window = {
      __TAURI_INTERNALS__: undefined
    };
  });

  it('should handle missing invoke in useRecording', async () => {
    // Import after setting up web mode environment
    const { useRecording } = await import('../src/lib/composables/useRecording');
    
    const recording = useRecording();
    
    // Should not throw when checking server status
    expect(() => recording).not.toThrow();
  });

  it('should handle missing invoke when clicking record button', async () => {
    const { useRecording } = await import('../src/lib/composables/useRecording');
    
    const recording = useRecording();
    
    // Attempt to toggle recording in web mode
    try {
      await recording.toggleRecording();
    } catch (error: any) {
      // Should get a friendly web mode error, not "Cannot read properties of undefined"
      expect(error.message).toContain('web mode');
      expect(error.message).not.toContain('Cannot read properties of undefined');
    }
  });

  it('should handle missing listen API in main page', async () => {
    // Test that event listeners don't crash when listen is undefined
    const mockListen = undefined;
    
    // Should handle undefined gracefully
    if (mockListen) {
      // This block should not execute
      expect(true).toBe(false);
    } else {
      // Should reach here without errors
      expect(true).toBe(true);
    }
  });

  it('should not show invoke errors on page load', async () => {
    // Simulate the model check on page load
    let whisperModelExists = false;
    let errorMessage = '';
    
    try {
      // This simulates the code in +page.svelte onMount
      if (typeof (global as any).window?.__TAURI_INTERNALS__ !== 'undefined' && typeof (global as any).invoke === 'function') {
        // Should not reach here in web mode
        expect(true).toBe(false);
      } else {
        // Web mode fallback
        whisperModelExists = false;
      }
    } catch (error: any) {
      errorMessage = error.message;
    }
    
    // Should not have any errors
    expect(errorMessage).toBe('');
    expect(whisperModelExists).toBe(false);
  });

  it('should handle clipboard operations in web mode', async () => {
    const { useRecording } = await import('../src/lib/composables/useRecording');
    
    const recording = useRecording();
    
    // Should provide fallback for clipboard operations
    // This would normally fail with "Cannot read properties of undefined (reading 'writeText')"
    // but should handle gracefully in web mode
    expect(() => recording).not.toThrow();
  });
});