// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Model Service - Handles all model-related operations
 * Single responsibility: Model management
 */

import { mustInvoke } from '$lib/ipc/invoke';
import { updateSetting } from '$lib/ipc/settings';
import {
  WhisperModelSize,
  getModelDisplayInfo,
  DownloadStatus,
  type ModelDownloadProgress,
} from '$lib/types/models';
import { ErrorFactory, ErrorHandler } from '$lib/types/errors';

/** Legacy interface for backend model list */
interface WhisperModelInfo {
  value: string;
  label: string;
  recommended_gpu?: string;
}

export interface ModelStatus {
  model: WhisperModelSize;
  isDownloaded: boolean;
  size?: number;
  path?: string;
}

/**
 * Service class for model operations
 */
export class ModelService {
  private static instance: ModelService;
  private downloadAbortController: AbortController | null = null;

  private constructor() {}

  static getInstance(): ModelService {
    if (!this.instance) {
      this.instance = new ModelService();
    }
    return this.instance;
  }

  /**
   * Get list of available models with their metadata
   */
  async getAvailableModels(): Promise<WhisperModelInfo[]> {
    try {
      const models = await mustInvoke<WhisperModelInfo[]>('get_available_models');
      return models;
    } catch (error) {
      throw await ErrorHandler.handle(error);
    }
  }

  /**
   * Check which models are currently downloaded
   */
  async checkDownloadedModels(): Promise<Map<WhisperModelSize, boolean>> {
    try {
      const statuses = await mustInvoke<[string, boolean][]>('check_models_exist');
      const modelMap = new Map<WhisperModelSize, boolean>();

      statuses.forEach(([model, isDownloaded]) => {
        // Convert string to enum
        const modelEnum = model as WhisperModelSize;
        if (Object.values(WhisperModelSize).includes(modelEnum)) {
          modelMap.set(modelEnum, isDownloaded);
        }
      });

      return modelMap;
    } catch (error) {
      throw await ErrorHandler.handle(error);
    }
  }

  /**
   * Check if a specific model is downloaded
   */
  async isModelDownloaded(model: WhisperModelSize): Promise<boolean> {
    const downloadedModels = await this.checkDownloadedModels();
    return downloadedModels.get(model) || false;
  }

  /**
   * Download a specific model
   */
  async downloadModel(
    model: WhisperModelSize,
    onProgress?: (progress: ModelDownloadProgress) => void
  ): Promise<void> {
    try {
      // Create new abort controller for this download
      this.downloadAbortController = new AbortController();

      // Update settings to selected model
      await this.selectModel(model);

      // Start download
      await mustInvoke('download_whisper_model');

      // Progress will be handled via Tauri events
      if (onProgress) {
        // Set up event listener for progress
        const unlisten = await this.listenToDownloadProgress(onProgress);

        // Clean up listener when done
        this.downloadAbortController.signal.addEventListener('abort', () => {
          unlisten();
        });
      }
    } catch (error) {
      if (error !== 'Download cancelled') {
        throw await ErrorHandler.handle(error);
      }
    }
  }

  /**
   * Cancel ongoing model download
   */
  async cancelDownload(): Promise<void> {
    try {
      await mustInvoke('cancel_model_download');
      this.downloadAbortController?.abort();
      this.downloadAbortController = null;
    } catch (error) {
      // Ignore cancellation errors
      console.warn('Failed to cancel download:', error);
    }
  }

  /**
   * Delete a downloaded model
   */
  async deleteModel(model: WhisperModelSize): Promise<void> {
    try {
      await mustInvoke('delete_model', { modelType: model });
    } catch (error) {
      throw await ErrorHandler.handle(error);
    }
  }

  /**
   * Select a model for use
   */
  async selectModel(model: WhisperModelSize): Promise<void> {
    try {
      await updateSetting('model', model);
    } catch (error) {
      throw await ErrorHandler.handle(error);
    }
  }

  /**
   * Get currently selected model
   */
  async getSelectedModel(): Promise<WhisperModelSize> {
    try {
      const settings = await mustInvoke<Record<string, unknown>>('get_settings');
      return settings.model as WhisperModelSize;
    } catch (error) {
      throw await ErrorHandler.handle(error);
    }
  }

  /**
   * Switch to a different model for download
   */
  async switchModelDownload(model: WhisperModelSize): Promise<void> {
    try {
      await mustInvoke('switch_model_download', { modelType: model });
    } catch (error) {
      throw await ErrorHandler.handle(error);
    }
  }

  /**
   * Listen to download progress events
   */
  private async listenToDownloadProgress(
    callback: (progress: ModelDownloadProgress) => void
  ): Promise<() => void> {
    const { listen } = await import('@tauri-apps/api/event');

    const unlistenProgress = await listen('model-download-progress', event => {
      callback(event.payload as ModelDownloadProgress);
    });

    const unlistenComplete = await listen('model-download-complete', () => {
      callback({
        progress: 100,
        speed: '',
        timeRemaining: '',
        timeElapsed: '',
        status: DownloadStatus.Complete,
      });
    });

    const unlistenError = await listen('model-download-error', event => {
      callback({
        progress: 0,
        speed: '',
        timeRemaining: '',
        timeElapsed: '',
        status: DownloadStatus.Error,
      });
    });

    // Return cleanup function
    return () => {
      unlistenProgress();
      unlistenComplete();
      unlistenError();
    };
  }

  /**
   * Validate model compatibility with system
   */
  async validateModelCompatibility(model: WhisperModelSize): Promise<{
    compatible: boolean;
    reason?: string;
  }> {
    // This could check GPU capabilities, available memory, etc.
    // For now, return true
    return { compatible: true };
  }

  /**
   * Get model recommendations based on system capabilities
   */
  async getRecommendedModel(): Promise<WhisperModelSize> {
    // TODO: Implement actual system detection
    // For now, return Medium as a safe default
    return WhisperModelSize.Medium;
  }
}

// Export singleton instance
export const modelService = ModelService.getInstance();
