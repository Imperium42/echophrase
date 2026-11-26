// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * @deprecated Use $lib/services/keyboardService instead.
 * This file is a backward-compatibility shim.
 */
export {
  isInitialized,
  lastError,
  convertToAccelerator,
  init,
  registerShortcut,
  isShortcutAvailable,
  unregisterCurrent,
  hasActiveGlobalShortcut,
  cleanup,
} from './keyboardService';
