// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Centralized Error Types and Handling
 */

import { APP_ENV } from '$lib/config/environment';

/** Human label for the current environment, used in user-visible messages. */
const ENV_LABEL: Record<typeof APP_ENV, string> = {
  development: 'Dev',
  staging: 'Staging',
  production: 'Prod',
};

/** Error severity levels */
export enum ErrorSeverity {
  Info = 'info',
  Warning = 'warning',
  Error = 'error',
  Critical = 'critical',
}

/** Error categories for better handling */
export enum ErrorCategory {
  Network = 'network',
  FileSystem = 'filesystem',
  Audio = 'audio',
  Model = 'model',
  Transcription = 'transcription',
  Settings = 'settings',
  Permission = 'permission',
  Unknown = 'unknown',
}

/** Base error interface */
export interface AppError {
  id: string;
  category: ErrorCategory;
  severity: ErrorSeverity;
  message: string;
  details?: string;
  timestamp: Date;
  recoverable: boolean;
  action?: ErrorAction;
}

/** Suggested actions for error recovery */
export interface ErrorAction {
  label: string;
  handler: () => void | Promise<void>;
}

/** Common error messages */
export const ERROR_MESSAGES = {
  // Network errors
  NETWORK_OFFLINE: 'No internet connection available',
  NETWORK_TIMEOUT: 'Request timed out. Please try again',
  NETWORK_SERVER_ERROR: 'Server error occurred. Please try again later',

  // Model errors
  MODEL_NOT_FOUND: 'Model not found. Please download it first',
  MODEL_DOWNLOAD_FAILED: 'Failed to download model',
  MODEL_CORRUPTED: 'Model file appears to be corrupted',
  MODEL_INCOMPATIBLE: 'Model is not compatible with current GPU',

  // Audio errors
  AUDIO_DEVICE_NOT_FOUND: 'No microphone detected',
  AUDIO_PERMISSION_DENIED: 'Microphone access denied',
  AUDIO_FORMAT_UNSUPPORTED: 'Audio format not supported',
  AUDIO_RECORDING_FAILED: 'Failed to record audio',

  // Transcription errors
  TRANSCRIPTION_FAILED: 'Failed to transcribe audio',
  TRANSCRIPTION_TIMEOUT: 'Transcription timed out',
  TRANSCRIPTION_EMPTY: 'No speech detected in audio',

  // File errors
  FILE_NOT_FOUND: 'File not found',
  FILE_TOO_LARGE: 'File size exceeds maximum limit',
  FILE_PERMISSION_DENIED: 'Permission denied to access file',
  FILE_WRITE_FAILED: 'Failed to save file',

  // Settings errors
  SETTINGS_LOAD_FAILED: 'Failed to load settings',
  SETTINGS_SAVE_FAILED: 'Failed to save settings',
  SETTINGS_INVALID: 'Invalid settings configuration',
} as const;

/** Error factory functions */
export class ErrorFactory {
  static create(category: ErrorCategory, message: string, options?: Partial<AppError>): AppError {
    return {
      id: crypto.randomUUID(),
      category,
      severity: options?.severity || ErrorSeverity.Error,
      message,
      details: options?.details,
      timestamp: new Date(),
      recoverable: options?.recoverable ?? true,
      action: options?.action,
    };
  }

  static networkError(message: string, recoverable = true): AppError {
    return this.create(ErrorCategory.Network, message, {
      severity: ErrorSeverity.Warning,
      recoverable,
    });
  }

  static modelError(message: string, details?: string): AppError {
    return this.create(ErrorCategory.Model, message, {
      severity: ErrorSeverity.Error,
      details,
      recoverable: true,
    });
  }

  static audioError(message: string): AppError {
    return this.create(ErrorCategory.Audio, message, {
      severity: ErrorSeverity.Error,
      recoverable: true,
    });
  }

  static transcriptionError(message: string, details?: string): AppError {
    return this.create(ErrorCategory.Transcription, message, {
      severity: ErrorSeverity.Error,
      details,
      recoverable: true,
    });
  }

  static criticalError(message: string, category = ErrorCategory.Unknown): AppError {
    return this.create(category, message, {
      severity: ErrorSeverity.Critical,
      recoverable: false,
    });
  }
}

/** Recording error type for RecordButton display */
export type RecordingErrorType =
  'model_missing' | 'gpu_error' | 'audio_device' | 'server_offline' | 'generic';

/** Structured recording error with optional action */
export interface RecordingError {
  message: string;
  type: RecordingErrorType;
  action?: {
    label: string;
    callback: () => void;
  };
}

/** Create a structured recording error from a raw error message */
export function classifyRecordingError(
  message: string,
  actions: {
    openDownloadModal?: () => void;
    openGpuSettings?: () => void;
    openAudioSettings?: () => void;
    retry?: () => void;
  } = {}
): RecordingError {
  const lower = message.toLowerCase();

  if (
    lower.includes('model') &&
    (lower.includes('not found') || lower.includes('download') || lower.includes('missing'))
  ) {
    return {
      message: 'Model missing',
      type: 'model_missing',
      action: actions.openDownloadModal
        ? { label: 'Download', callback: actions.openDownloadModal }
        : undefined,
    };
  }

  if (lower.includes('gpu') || lower.includes('directml') || lower.includes('onnx')) {
    return {
      message: 'GPU unavailable',
      type: 'gpu_error',
      action: actions.openGpuSettings
        ? { label: 'GPU Settings', callback: actions.openGpuSettings }
        : undefined,
    };
  }

  if (
    lower.includes('microphone') ||
    lower.includes('audio') ||
    lower.includes('mic') ||
    lower.includes('input device')
  ) {
    return {
      message: 'Microphone not detected',
      type: 'audio_device',
      action: actions.openAudioSettings
        ? { label: 'Audio Settings', callback: actions.openAudioSettings }
        : undefined,
    };
  }

  if (lower.includes('server') || lower.includes('offline') || lower.includes('connection')) {
    return {
      message: `${ENV_LABEL[APP_ENV]} server offline`,
      type: 'server_offline',
      action: actions.retry ? { label: 'Retry', callback: actions.retry } : undefined,
    };
  }

  return {
    message,
    type: 'generic',
  };
}

/** Error handler utility */
export class ErrorHandler {
  static async handle(error: unknown): Promise<AppError> {
    // Handle Tauri errors
    if (error && typeof error === 'object' && 'message' in error) {
      const message = (error as any).message || 'Unknown error occurred';

      // Categorize based on error message
      if (message.includes('network') || message.includes('timeout')) {
        return ErrorFactory.networkError(message);
      }
      if (message.includes('model') || message.includes('download')) {
        return ErrorFactory.modelError(message);
      }
      if (message.includes('audio') || message.includes('microphone')) {
        return ErrorFactory.audioError(message);
      }
      if (message.includes('transcri')) {
        return ErrorFactory.transcriptionError(message);
      }
    }

    // Handle string errors
    if (typeof error === 'string') {
      return ErrorFactory.create(ErrorCategory.Unknown, error);
    }

    // Default unknown error
    return ErrorFactory.create(ErrorCategory.Unknown, 'An unexpected error occurred', {
      details: String(error),
    });
  }

  static isRecoverable(error: AppError): boolean {
    return error.recoverable && error.severity !== ErrorSeverity.Critical;
  }

  static getSuggestion(error: AppError): string | null {
    switch (error.category) {
      case ErrorCategory.Network:
        return 'Check your internet connection and try again';
      case ErrorCategory.Model:
        return 'Try downloading the model again';
      case ErrorCategory.Audio:
        return 'Check your microphone settings and permissions';
      case ErrorCategory.Transcription:
        return 'Try recording again with clearer audio';
      case ErrorCategory.Permission:
        return 'Grant the necessary permissions in your system settings';
      default:
        return null;
    }
  }
}
