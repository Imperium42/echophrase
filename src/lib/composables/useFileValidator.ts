// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Modern composable for file validation
 * Handles file type and size validation for audio files
 */
import { translate } from '$lib/locales';

export function useFileValidator() {
  const acceptedTypes = [
    'audio/wav',
    'audio/mp3',
    'audio/mpeg',
    'audio/m4a',
    'audio/flac',
    'audio/ogg',
    'audio/webm',
  ];

  const maxSize = 100 * 1024 * 1024; // 100MB in bytes

  /**
   * Validate file type and size
   */
  function validateFile(file: File): { isValid: boolean; error?: string } {
    // Validate file type
    if (!acceptedTypes.includes(file.type)) {
      return {
        isValid: false,
        error: translate('errors.invalidAudioFile'),
      };
    }

    // Validate file size
    if (file.size > maxSize) {
      return {
        isValid: false,
        error: translate('errors.fileTooLarge'),
      };
    }

    return { isValid: true };
  }

  /**
   * Get human-readable file size
   */
  function formatFileSize(bytes: number): string {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    return `${size.toFixed(1)} ${units[unitIndex]}`;
  }

  /**
   * Check if file type is supported
   */
  function isFileTypeSupported(file: File): boolean {
    return acceptedTypes.includes(file.type);
  }

  /**
   * Check if file size is within limits
   */
  function isFileSizeValid(file: File): boolean {
    return file.size <= maxSize;
  }

  return {
    acceptedTypes,
    maxSize,
    validateFile,
    formatFileSize,
    isFileTypeSupported,
    isFileSizeValid,
  };
}
