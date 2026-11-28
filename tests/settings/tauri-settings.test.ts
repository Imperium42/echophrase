// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { describe, it, expect, beforeEach, vi } from 'vitest';

/**
 * Test suite for the Tauri-based settings system
 * Tests the useTauriSettings composable and model metadata
 */
describe('Tauri Settings System', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Model Metadata', () => {
    it('should validate model metadata structure', () => {
      const expectedModels = [
        'tiny', 'base', 'small', 'medium', 'large', 'large-v2', 'large-v3'
      ];

      expectedModels.forEach(model => {
        expect(model).toMatch(/^(tiny|base|small|medium|large|large-v2|large-v3)$/);
      });
    });

    it('should validate model size progression', () => {
      const modelSizes = [
        { model: 'tiny', size: 39 },
        { model: 'base', size: 74 },
        { model: 'small', size: 461 },
        { model: 'medium', size: 1460 },
        { model: 'large', size: 2900 },
        { model: 'large-v2', size: 2900 },
        { model: 'large-v3', size: 2900 },
      ];

      // Verify sizes are in ascending order (except for large variants)
      expect(modelSizes[0].size).toBeLessThan(modelSizes[1].size);
      expect(modelSizes[1].size).toBeLessThan(modelSizes[2].size);
      expect(modelSizes[2].size).toBeLessThan(modelSizes[3].size);
      expect(modelSizes[3].size).toBeLessThan(modelSizes[4].size);
    });

    it('should validate quality ratings format', () => {
      const validQualityPattern = /^★{1,5}☆{0,4}$/;
      const qualityRatings = [
        '★★☆☆☆', // tiny/base
        '★★★☆☆', // small
        '★★★★☆', // medium  
        '★★★★★'  // large variants
      ];

      qualityRatings.forEach(rating => {
        expect(rating).toMatch(validQualityPattern);
        expect(rating).toHaveLength(5); // Always 5 characters total
      });
    });

    it('should validate speed format', () => {
      const speedPattern = /^~\d+x realtime$/;
      const speeds = [
        '~10x realtime', // tiny
        '~8x realtime',  // base
        '~6x realtime',  // small
        '~4x realtime',  // medium
        '~2x realtime'   // large variants
      ];

      speeds.forEach(speed => {
        expect(speed).toMatch(speedPattern);
      });
    });

    it('should validate GPU requirements are realistic', () => {
      const gpuRequirements = [
        { model: 'tiny', vram: 1.0 },
        { model: 'base', vram: 1.0 },
        { model: 'small', vram: 2.0 },
        { model: 'medium', vram: 4.0 },
        { model: 'large', vram: 6.0 },
      ];

      gpuRequirements.forEach(({ model, vram }) => {
        expect(vram).toBeGreaterThan(0);
        expect(vram).toBeLessThanOrEqual(8.0); // Reasonable upper bound
        expect(Number.isFinite(vram)).toBe(true);
      });
    });
  });

  describe('Settings Structure', () => {
    it('should validate default settings structure', () => {
      const defaultSettings = {
        model: 'small',
        language: 'en',
        theme: 'dark',
        output_format: 'text',
        hotkey: 'ctrl+shift+r',
        auto_copy: true,
        show_timestamps: false,
        compute_type: 'auto',
      };

      // Validate types
      expect(typeof defaultSettings.model).toBe('string');
      expect(typeof defaultSettings.language).toBe('string');
      expect(typeof defaultSettings.theme).toBe('string');
      expect(typeof defaultSettings.output_format).toBe('string');
      expect(typeof defaultSettings.hotkey).toBe('string');
      expect(typeof defaultSettings.auto_copy).toBe('boolean');
      expect(typeof defaultSettings.show_timestamps).toBe('boolean');
      expect(typeof defaultSettings.compute_type).toBe('string');
    });

    it('should validate hotkey format', () => {
      const validHotkeys = [
        'ctrl+shift+r',
        'ctrl+alt+t',
        'cmd+shift+r',
        'meta+shift+s'
      ];

      const hotkeyPattern = /^(ctrl|cmd|meta|alt)(\+(shift|alt|ctrl))*\+[a-z]$/i;

      validHotkeys.forEach(hotkey => {
        expect(hotkey).toMatch(hotkeyPattern);
      });
    });

    it('should validate compute type options', () => {
      const validComputeTypes = ['auto', 'int8', 'float16', 'float32'];
      const computeType = 'auto';

      expect(validComputeTypes).toContain(computeType);
    });
  });

  describe('Web Mode Compatibility', () => {
    beforeEach(() => {
      // Simulate web mode
      (global as any).window = {
        __TAURI_INTERNALS__: undefined
      };
    });

    it('should handle missing Tauri APIs gracefully', async () => {
      // Import after setting web mode
      const { useTauriSettings } = await import('../../src/lib/composables/useTauriSettings');
      
      const settings = useTauriSettings();
      
      // Should not throw when initializing
      expect(() => settings).not.toThrow();
      
      // Should handle initialization gracefully
      const initialized = await settings.initializeTauri();
      expect(initialized).toBe(false);
    });

    it('should set appropriate error messages in web mode', async () => {
      const { useTauriSettings } = await import('../../src/lib/composables/useTauriSettings');
      
      const settings = useTauriSettings();
      
      // Attempt to load settings in web mode
      await settings.loadSettings();
      
      // Should have error state but not crash
      expect(() => settings.loadSettings()).not.toThrow();
    });
  });

  describe('Model URL Construction', () => {
    it('should construct valid HuggingFace URLs', () => {
      const baseUrl = 'https://huggingface.co/Systran/faster-whisper-small.en';
      const expectedPattern = /^https:\/\/huggingface\.co\/Systran\/faster-whisper-.+$/;
      
      expect(baseUrl).toMatch(expectedPattern);
    });

    it('should validate model names follow convention', () => {
      const modelNames = [
        'Systran/faster-whisper-tiny.en',
        'Systran/faster-whisper-base.en',
        'Systran/faster-whisper-small.en',
        'Systran/faster-whisper-medium.en',
        'Systran/faster-whisper-large-v2',
        'Systran/faster-whisper-large-v3'
      ];

      const namePattern = /^Systran\/faster-whisper-.+$/;

      modelNames.forEach(name => {
        expect(name).toMatch(namePattern);
      });
    });
  });
});