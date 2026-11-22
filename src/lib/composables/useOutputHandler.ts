// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { useClipboard } from './useClipboard';
import { tauriSettings } from './useTauriSettings';
import { get } from 'svelte/store';
import { isWSLAsync } from '$lib/utils/platform';
import {
  isAccessibilityError,
  refreshAccessibilityPermission,
  showAccessibilityModal,
} from './useAccessibilityPermission';

/**
 * Output destination modes for transcription results
 */
export type OutputMode = 'paste_at_cursor' | 'copy_to_clipboard' | 'save_as_file';

// Localized output modes are in settings.ts - use getAvailableOutputModes(t)
export { getAvailableOutputModes } from '$lib/types/settings';

// Dynamic Tauri invoke reference
let invoke: any = null;

/**
 * Initialize Tauri API
 */
async function initializeTauri(): Promise<boolean> {
  if (invoke) return true;

  if (typeof window !== 'undefined' && typeof (window as any).__TAURI_INTERNALS__ !== 'undefined') {
    try {
      const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
      invoke = tauriInvoke;
      return true;
    } catch (error) {
      console.warn('Failed to load Tauri APIs for output handler:', error);
      return false;
    }
  }
  return false;
}

/**
 * Composable for handling transcription output to various destinations
 * Supports: paste at cursor, copy to clipboard, save to file
 */
/** Wait after the paste keystroke before restoring the user's previous clipboard. */
const CLIPBOARD_RESTORE_DELAY_MS = 400;

export function useOutputHandler() {
  const clipboard = useClipboard();

  /**
   * Handle output based on the selected mode
   * @param text The formatted text to output
   * @param mode The output mode
   * @param filename Optional filename for save_as_file mode
   */
  async function handleOutput(
    text: string,
    mode: OutputMode,
    filename?: string,
    keepOnClipboard = true
  ): Promise<{ success: boolean; error?: string }> {
    console.log(`📤 Handling output with mode: ${mode}`);

    switch (mode) {
      case 'paste_at_cursor':
        return await pasteAtCursor(text, keepOnClipboard);

      case 'copy_to_clipboard':
        return await copyToClipboard(text);

      case 'save_as_file':
        return await saveAsFile(text, filename);

      default:
        console.warn(`Unknown output mode: ${mode}`);
        return { success: false, error: `Unknown output mode: ${mode}` };
    }
  }

  /**
   * Copy text to clipboard and simulate paste (Ctrl+V)
   *
   * WSL requires special handling: the Tauri clipboard plugin writes to the
   * Linux/Wayland clipboard which doesn't bridge to Windows. We use clip.exe
   * via the `copy_to_windows_clipboard` command instead, and powershell.exe
   * SendKeys for the keystroke (handled in Rust simulate_paste).
   *
   * Pasting has to go through the clipboard. Unless `keepOnClipboard` (the
   * user ALSO chose "Copy to clipboard"), the previous clipboard text is put
   * back after the paste, so "Paste at cursor" alone never leaves the
   * transcript on the clipboard. Not on WSL (clip.exe can't be read back), and
   * a non-text clipboard (eg an image) can't be restored - it is left cleared.
   * On a paste failure the transcript is deliberately left on the clipboard
   * so the user can paste it by hand.
   */
  async function pasteAtCursor(
    text: string,
    keepOnClipboard = true
  ): Promise<{ success: boolean; error?: string }> {
    try {
      const initialized = await initializeTauri();
      if (!initialized) {
        return {
          success: false,
          error: 'Tauri not available - cannot simulate paste in web mode',
        };
      }

      // WSL: use clip.exe to bridge to Windows clipboard
      // Native: use Tauri clipboard plugin
      const wsl = await isWSLAsync();
      if (wsl) {
        await invoke('copy_to_windows_clipboard', { text });
        console.log('📋 Text copied to Windows clipboard via clip.exe');
      }
      const previous = !wsl && !keepOnClipboard ? await clipboard.readClipboard() : null;
      if (!wsl) {
        await clipboard.copyToClipboard(text);
        console.log('📋 Text copied to clipboard');
      }

      // Simulate paste keystroke (WSL-aware on Rust side)
      await invoke('simulate_paste');
      console.log('⌨️ Paste keystroke simulated');

      if (!wsl && !keepOnClipboard) {
        // The target app reads the clipboard asynchronously after Ctrl+V;
        // restoring too early would paste the OLD content.
        await new Promise(resolve => setTimeout(resolve, CLIPBOARD_RESTORE_DELAY_MS));
        await clipboard.copyToClipboard(previous ?? '');
      }

      return { success: true };
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);

      // macOS: keystroke synthesis is gated behind Privacy & Security >
      // Accessibility. Surface our own explanation of WHY a dictation app needs
      // it rather than the OS's context-free "control this computer" prompt.
      // The transcript is already on the clipboard, so the user can Cmd+V
      // manually in the meantime.
      if (isAccessibilityError(error)) {
        console.warn('⚠️ Paste blocked: Accessibility permission not granted');
        await refreshAccessibilityPermission();
        showAccessibilityModal.set(true);
        return { success: false, error: errorMsg };
      }

      console.error('❌ Failed to paste at cursor:', errorMsg);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Copy text to clipboard only
   */
  async function copyToClipboard(text: string): Promise<{ success: boolean; error?: string }> {
    try {
      await clipboard.copyToClipboard(text);
      console.log('📋 Text copied to clipboard');
      return { success: true };
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      console.error('❌ Failed to copy to clipboard:', errorMsg);
      return { success: false, error: errorMsg };
    }
  }

  /**
   * Save text to file - auto-saves to configured directory, or falls back to dialog
   */
  async function saveAsFile(
    text: string,
    suggestedFilename?: string
  ): Promise<{ success: boolean; error?: string; savedPath?: string }> {
    try {
      const initialized = await initializeTauri();
      if (!initialized) {
        return {
          success: false,
          error: 'Tauri not available - cannot save file in web mode',
        };
      }

      const settings = get(tauriSettings);
      const fileExt = settings?.output_format === 'json' ? 'json' : 'txt';
      const filename = suggestedFilename || `transcript_${Date.now()}.${fileExt}`;

      // Auto-save to configured directory if set
      if (settings?.save_directory) {
        const savedPath = (await invoke('save_transcript_to_directory', {
          content: text,
          filename,
          directory: settings.save_directory,
        })) as string;
        console.log('Transcript auto-saved to:', savedPath);
        return { success: true, savedPath };
      }

      // Fall back to save dialog
      const result = (await invoke('save_transcript_dialog', { content: text, filename })) as
        string | null;

      if (result) {
        console.log('Transcript saved to file:', result);
        return { success: true, savedPath: result };
      } else {
        // User cancelled the dialog
        console.log('Save dialog cancelled');
        return { success: true }; // Not an error, just cancelled
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      console.error('Failed to save file:', errorMsg);
      return { success: false, error: errorMsg };
    }
  }

  return {
    handleOutput,
    pasteAtCursor,
    copyToClipboard,
    saveAsFile,
  };
}
