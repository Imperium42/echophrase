// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { writeText } from '@tauri-apps/plugin-clipboard-manager';

// Cached writeText function for HMR recovery
let cachedWriteText: typeof writeText | null = writeText;

/**
 * Modern composable for clipboard operations
 * Handles copying text to system clipboard using Tauri's native clipboard manager
 * Includes HMR recovery and retry logic for resilience
 */
export function useClipboard() {
  /**
   * Get writeText function, re-importing if necessary (HMR recovery)
   */
  async function getWriteText(): Promise<typeof writeText> {
    if (!cachedWriteText) {
      console.log('📋 Re-importing clipboard API (HMR recovery)...');
      const clipboard = await import('@tauri-apps/plugin-clipboard-manager');
      cachedWriteText = clipboard.writeText;
    }
    return cachedWriteText;
  }

  /**
   * Copy text to clipboard using Tauri's native clipboard manager
   * Includes retry logic with fresh import on failure
   */
  async function copyToClipboard(text: string): Promise<void> {
    try {
      const write = await getWriteText();
      await write(text);
      console.log('📋 Copied to clipboard successfully');
    } catch (error) {
      console.error('❌ Clipboard write failed:', error);
      // Retry once with fresh import
      try {
        console.log('📋 Retrying clipboard write with fresh import...');
        const clipboard = await import('@tauri-apps/plugin-clipboard-manager');
        cachedWriteText = clipboard.writeText;
        await cachedWriteText(text);
        console.log('✅ Clipboard write succeeded on retry');
      } catch (retryError) {
        console.error('❌ Clipboard write retry also failed:', retryError);
        throw retryError;
      }
    }
  }

  /**
   * Copy text with success/error feedback
   */
  async function copyWithFeedback(
    text: string,
    onSuccess?: () => void,
    onError?: (error: Error) => void
  ): Promise<void> {
    try {
      await copyToClipboard(text);
      onSuccess?.();
    } catch (error) {
      const errorObj = error instanceof Error ? error : new Error('Failed to copy to clipboard');
      onError?.(errorObj);
    }
  }

  return {
    copyToClipboard,
    copyWithFeedback,
  };
}
