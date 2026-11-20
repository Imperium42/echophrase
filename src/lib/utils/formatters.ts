// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Formatting Utilities
 * Centralized formatting functions for consistency
 */

/**
 * Format file size to human-readable string
 */
export function formatFileSize(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let size = Math.abs(bytes);
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }

  // Use appropriate decimal places based on size
  const decimals = unitIndex === 0 ? 0 : size < 10 ? 2 : 1;
  return `${size.toFixed(decimals)} ${units[unitIndex]}`;
}

/**
 * Format duration to human-readable string
 */
export function formatDuration(seconds: number): string {
  if (seconds < 0) return '0s';

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return secs > 0 ? `${minutes}m ${secs}s` : `${minutes}m`;
  }
  return `${secs}s`;
}

/**
 * Format speed to human-readable string
 */
export function formatSpeed(bytesPerSecond: number): string {
  if (bytesPerSecond <= 0) return '';

  const mbps = bytesPerSecond / (1024 * 1024);
  if (mbps < 1) {
    const kbps = bytesPerSecond / 1024;
    return `${kbps.toFixed(1)} KB/s`;
  }
  return `${mbps.toFixed(1)} MB/s`;
}

/**
 * Format percentage with appropriate precision
 */
export function formatPercentage(value: number, decimals = 0): string {
  const percentage = Math.min(100, Math.max(0, value));
  return `${percentage.toFixed(decimals)}%`;
}

/**
 * Format timestamp to relative time
 */
export function formatRelativeTime(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays > 0) return `${diffDays}d ago`;
  if (diffHours > 0) return `${diffHours}h ago`;
  if (diffMins > 0) return `${diffMins}m ago`;
  if (diffSecs > 0) return `${diffSecs}s ago`;
  return 'just now';
}

/**
 * Format model name for display
 */
export function formatModelName(modelId: string): string {
  // Remove version suffixes and format
  const name = modelId
    .replace(/-v\d+$/, '')
    .replace(/-/g, ' ')
    .replace(/\b\w/g, l => l.toUpperCase());

  return name;
}

/**
 * Parse time string to seconds
 */
export function parseTimeString(timeStr: string): number {
  if (!timeStr) return 0;

  const patterns = [
    { regex: /(\d+)h\s*(\d+)m/, multipliers: [3600, 60] },
    { regex: /(\d+)m\s*(\d+)s/, multipliers: [60, 1] },
    { regex: /(\d+)h/, multipliers: [3600] },
    { regex: /(\d+)m/, multipliers: [60] },
    { regex: /(\d+)s/, multipliers: [1] },
    { regex: /(\d+)/, multipliers: [1] }, // Assume seconds if no unit
  ];

  for (const pattern of patterns) {
    const match = timeStr.match(pattern.regex);
    if (match) {
      let total = 0;
      for (let i = 1; i < match.length && i - 1 < pattern.multipliers.length; i++) {
        total += parseInt(match[i]) * pattern.multipliers[i - 1];
      }
      return total;
    }
  }

  return 0;
}

/**
 * Truncate text with ellipsis
 */
export function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return text.substring(0, maxLength - 3) + '...';
}

/**
 * Format number with thousand separators
 */
export function formatNumber(num: number): string {
  return num.toLocaleString('en-US');
}

/**
 * Capitalize first letter
 */
export function capitalize(str: string): string {
  if (!str) return '';
  return str.charAt(0).toUpperCase() + str.slice(1).toLowerCase();
}

/**
 * Format GPU name for display
 */
export function formatGpuName(gpu: string): string {
  // Add spaces before numbers if needed
  return gpu.replace(/([A-Z])(\d)/g, '$1 $2');
}

/**
 * Format an ISO timestamp as a long, date-only string in the APP locale (not the
 * WebView's). Passing `undefined` to `toLocaleDateString` picks the OS/WebView language,
 * so a zh-TW user on an English OS saw "September 3, 2027" inside Chinese copy. The time
 * of day is dropped on purpose: it is noise for a billing boundary.
 */
export function formatLongDate(iso: string, locale: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  try {
    return new Intl.DateTimeFormat(locale, { dateStyle: 'long' }).format(date);
  } catch {
    return date.toISOString().slice(0, 10);
  }
}
