// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Download Manager Service
 * Single responsibility: Managing download operations and progress
 */

import { writable, derived, get } from 'svelte/store';
import { DownloadStatus } from '$lib/types/models';
import type { ModelDownloadProgress } from '$lib/types/models';

export interface DownloadTask {
  id: string;
  name: string;
  progress: number;
  speed: string;
  timeRemaining: string;
  timeElapsed: string;
  status: DownloadStatus;
  startTime: Date;
  endTime?: Date;
  error?: string;
}

export interface DownloadStats {
  totalDownloads: number;
  successfulDownloads: number;
  failedDownloads: number;
  totalBytesDownloaded: number;
  averageSpeed: number;
}

/**
 * Download Manager - handles all download operations
 */
export class DownloadManager {
  private static instance: DownloadManager;

  // Stores
  private currentDownload = writable<DownloadTask | null>(null);
  private downloadHistory = writable<DownloadTask[]>([]);
  private downloadStats = writable<DownloadStats>({
    totalDownloads: 0,
    successfulDownloads: 0,
    failedDownloads: 0,
    totalBytesDownloaded: 0,
    averageSpeed: 0,
  });

  // Derived stores
  public isDownloading = derived(
    this.currentDownload,
    $current => $current !== null && $current.status === DownloadStatus.Downloading
  );

  public downloadProgress = derived(this.currentDownload, $current => $current?.progress || 0);

  private constructor() {
    this.loadStats();
  }

  static getInstance(): DownloadManager {
    if (!this.instance) {
      this.instance = new DownloadManager();
    }
    return this.instance;
  }

  /**
   * Start a new download task
   */
  startDownload(name: string): string {
    const taskId = crypto.randomUUID();
    const task: DownloadTask = {
      id: taskId,
      name,
      progress: 0,
      speed: '',
      timeRemaining: '',
      timeElapsed: '',
      status: DownloadStatus.Connecting,
      startTime: new Date(),
    };

    this.currentDownload.set(task);
    this.updateStats(stats => ({
      ...stats,
      totalDownloads: stats.totalDownloads + 1,
    }));

    return taskId;
  }

  /**
   * Update download progress
   */
  updateProgress(progress: ModelDownloadProgress): void {
    this.currentDownload.update(task => {
      if (!task) return null;

      return {
        ...task,
        progress: progress.progress,
        speed: progress.speed,
        timeRemaining: progress.timeRemaining,
        timeElapsed: progress.timeElapsed,
        status: progress.status,
      };
    });

    // Handle completion
    if (progress.status === DownloadStatus.Complete) {
      this.completeDownload();
    } else if (progress.status === DownloadStatus.Error) {
      this.failDownload('Download failed');
    }
  }

  /**
   * Complete current download
   */
  private completeDownload(): void {
    const current = get(this.currentDownload);
    if (!current) return;

    const completed: DownloadTask = {
      ...current,
      status: DownloadStatus.Complete,
      progress: 100,
      endTime: new Date(),
    };

    this.downloadHistory.update(history => [...history, completed]);
    this.updateStats(stats => ({
      ...stats,
      successfulDownloads: stats.successfulDownloads + 1,
    }));

    // Clear current after a delay
    setTimeout(() => {
      this.currentDownload.set(null);
    }, 2000);
  }

  /**
   * Mark download as failed
   */
  failDownload(error: string): void {
    const current = get(this.currentDownload);
    if (!current) return;

    const failed: DownloadTask = {
      ...current,
      status: DownloadStatus.Error,
      error,
      endTime: new Date(),
    };

    this.downloadHistory.update(history => [...history, failed]);
    this.updateStats(stats => ({
      ...stats,
      failedDownloads: stats.failedDownloads + 1,
    }));

    this.currentDownload.update(task => ({
      ...failed,
    }));
  }

  /**
   * Cancel current download
   */
  cancelDownload(): void {
    const current = get(this.currentDownload);
    if (!current) return;

    const cancelled: DownloadTask = {
      ...current,
      status: DownloadStatus.Cancelled,
      endTime: new Date(),
    };

    this.downloadHistory.update(history => [...history, cancelled]);
    this.currentDownload.set(null);
  }

  /**
   * Get download history
   */
  getHistory(): DownloadTask[] {
    return get(this.downloadHistory);
  }

  /**
   * Clear download history
   */
  clearHistory(): void {
    this.downloadHistory.set([]);
  }

  /**
   * Calculate average download speed
   */
  private calculateAverageSpeed(): number {
    const history = get(this.downloadHistory);
    const recentDownloads = history.filter(d => d.status === DownloadStatus.Complete).slice(-10); // Last 10 successful downloads

    if (recentDownloads.length === 0) return 0;

    const speeds = recentDownloads
      .map(d => {
        // Parse speed from "X.X MB/s" format
        const match = d.speed.match(/(\d+\.?\d*)/);
        return match ? parseFloat(match[1]) : 0;
      })
      .filter(s => s > 0);

    return speeds.length > 0 ? speeds.reduce((a, b) => a + b, 0) / speeds.length : 0;
  }

  /**
   * Update stats
   */
  private updateStats(updater: (stats: DownloadStats) => DownloadStats): void {
    this.downloadStats.update(updater);
    this.saveStats();
  }

  /**
   * Load stats from localStorage
   */
  private loadStats(): void {
    if (typeof window === 'undefined') return;

    const saved = localStorage.getItem('download-stats');
    if (saved) {
      try {
        const stats = JSON.parse(saved);
        this.downloadStats.set(stats);
      } catch (error) {
        console.warn('Failed to load download stats:', error);
      }
    }
  }

  /**
   * Save stats to localStorage
   */
  private saveStats(): void {
    if (typeof window === 'undefined') return;

    const stats = get(this.downloadStats);
    localStorage.setItem('download-stats', JSON.stringify(stats));
  }

  /**
   * Format bytes to human-readable size
   */
  static formatBytes(bytes: number): string {
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
   * Format time duration
   */
  static formatDuration(seconds: number): string {
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return remainingSeconds > 0 ? `${minutes}m ${remainingSeconds}s` : `${minutes}m`;
  }

  /**
   * Parse time string to seconds
   */
  static parseTimeString(timeStr: string): number {
    const match = timeStr.match(/(\d+)([ms])/);
    if (!match) return 0;

    const [, value, unit] = match;
    const num = parseInt(value);
    return unit === 'm' ? num * 60 : num;
  }
}

// Export singleton instance
export const downloadManager = DownloadManager.getInstance();
