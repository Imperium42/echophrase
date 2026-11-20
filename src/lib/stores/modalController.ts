// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Modal Layer Controller
 *
 * Centralized modal state management with keyboard support.
 * Manages z-index layering and ESC key handling for consistent UX.
 *
 * Features:
 * - Stack-based modal management (LIFO)
 * - Global ESC listener closes top modal
 * - Auto z-index calculation (base 9000 + 10 per layer)
 * - Focus management hints for accessibility
 *
 * Usage:
 * ```typescript
 * import { modalController } from '$lib/stores/modalController';
 *
 * // Register modal on mount
 * const modalId = modalController.push({
 *   onClose: () => visible = false,
 *   priority: 0
 * });
 *
 * // Unregister on destroy
 * onDestroy(() => modalController.pop(modalId));
 * ```
 */

import { writable, derived, get } from 'svelte/store';

// Browser detection - simpler than importing from $app/environment
const browser = typeof window !== 'undefined';

/** Modal priority tiers - determines base z-index and ESC ordering */
export enum ModalPriority {
  /** Background overlays like onboarding (base z-index: 100) */
  BACKGROUND = 0,
  /** Standard modals: settings, model download (base z-index: 200) */
  NORMAL = 1,
  /** High-priority modals that overlay other modals: CUDA setup, confirm (base z-index: 300) */
  HIGH = 2,
  /** Blocking overlays: install/loading (base z-index: 9999) */
  BLOCKING = 3,
}

export interface ModalLayer {
  id: string;
  onClose: () => void;
  priority: ModalPriority;
  zIndex: number;
  timestamp: number;
  /** If false, ESC skips this modal (eg onboarding on step 0) */
  allowEsc: boolean;
}

interface ModalStore {
  layers: ModalLayer[];
}

// Z-index layers architecture (priority-tiered):
// - Debug tools: 10000 (always on top, never blocked)
// - BLOCKING overlays: 9999 (install/loading)
// - HIGH modals: 300+ (cuda setup, confirm)
// - NORMAL modals: 200+ (settings, model download)
// - BACKGROUND overlays: 100+ (onboarding)
// - Main content: default (< 100)
const PRIORITY_BASE_Z: Record<ModalPriority, number> = {
  [ModalPriority.BACKGROUND]: 100,
  [ModalPriority.NORMAL]: 200,
  [ModalPriority.HIGH]: 300,
  [ModalPriority.BLOCKING]: 9999,
};
const Z_INDEX_INCREMENT = 10;
const DEBUG_TOOLS_Z_INDEX = 10000;

/**
 * Global overlay blocking state
 * When true, all ESC key handling is blocked (e.g., during an install)
 */
let _isGlobalOverlayBlocking = false;

/**
 * Check if global overlay is currently blocking user input
 * @returns true if global overlay is visible and blocking
 */
export function isGlobalOverlayBlocking(): boolean {
  return _isGlobalOverlayBlocking;
}

/**
 * Set global overlay blocking state
 * Called by LoadingOverlay component to control ESC blocking
 * @param blocking true to block all ESC handling
 */
export function setGlobalOverlayBlocking(blocking: boolean): void {
  _isGlobalOverlayBlocking = blocking;
}

function createModalController() {
  const { subscribe, set, update } = writable<ModalStore>({ layers: [] });

  // Derived store for top modal
  const topModal = derived({ subscribe }, $store => {
    if ($store.layers.length === 0) return null;
    // Sort by priority (descending), then by timestamp (descending)
    const sorted = [...$store.layers].sort((a, b) => {
      if (b.priority !== a.priority) return b.priority - a.priority;
      return b.timestamp - a.timestamp;
    });
    return sorted[0];
  });

  // Global ESC key listener (only in browser)
  // This is the SINGLE source of truth for ESC modal closing.
  // No other component should handle ESC for modal dismissal.
  if (browser) {
    window.addEventListener('keydown', (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;

      const store = get({ subscribe });
      if (store.layers.length === 0) return;

      e.preventDefault();
      e.stopImmediatePropagation();

      // Find topmost modal that allows ESC (sorted by z-index descending)
      const sorted = [...store.layers].sort((a, b) => b.zIndex - a.zIndex);
      const escapable = sorted.find(layer => layer.allowEsc);
      if (escapable) {
        escapable.onClose();
      }
    });
  }

  /** Whether any modal is currently open */
  const isAnyModalOpen = derived({ subscribe }, $store => $store.layers.length > 0);

  /** ID of the topmost modal (by z-index) */
  const topModalId = derived(topModal, $top => $top?.id ?? null);

  /** Number of open modals */
  const modalCount = derived({ subscribe }, $store => $store.layers.length);

  /** Check if a specific modal is open */
  function isOpen(id: string): boolean {
    return get({ subscribe }).layers.some(l => l.id === id);
  }

  return {
    subscribe,
    topModal,
    isAnyModalOpen,
    topModalId,
    modalCount,
    isOpen,

    /**
     * Register a new modal layer
     * @param config Modal configuration
     * @returns Modal ID for later removal
     */
    push(config: {
      id?: string;
      onClose: () => void;
      priority?: ModalPriority;
      allowEsc?: boolean;
    }): string {
      const id = config.id ?? `modal-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
      const timestamp = Date.now();
      const priority = config.priority ?? ModalPriority.NORMAL;
      const allowEsc = config.allowEsc ?? true;

      update(store => {
        // Remove existing layer with same id (prevent duplicates)
        const filtered = store.layers.filter(l => l.id !== id);

        // Calculate z-index: base for priority tier + increment per layer in that tier
        const sameTierCount = filtered.filter(l => l.priority === priority).length;
        const zIndex = PRIORITY_BASE_Z[priority] + sameTierCount * Z_INDEX_INCREMENT;

        const newLayer: ModalLayer = {
          id,
          onClose: config.onClose,
          priority,
          zIndex,
          timestamp,
          allowEsc,
        };

        return {
          layers: [...filtered, newLayer],
        };
      });

      return id;
    },

    /**
     * Remove a modal layer by ID
     * @param id Modal ID returned from push()
     */
    pop(id: string): void {
      update(store => ({
        layers: store.layers.filter(layer => layer.id !== id),
      }));
    },

    /**
     * Close the top-most modal
     */
    closeTop(): void {
      const top = get(topModal);
      if (top) {
        top.onClose();
      }
    },

    /**
     * Close all modals
     */
    closeAll(): void {
      const currentStore = get({ subscribe });
      currentStore.layers.forEach(layer => layer.onClose());
      set({ layers: [] });
    },

    /**
     * Get z-index for a specific modal ID
     * @param id Modal ID
     * @returns z-index value or null if not found
     */
    getZIndex(id: string): number | null {
      const currentStore = get({ subscribe });
      const layer = currentStore.layers.find(l => l.id === id);
      return layer?.zIndex ?? null;
    },
  };
}

export const modalController = createModalController();

/**
 * Z-index constants for consistent layering across the app
 */
export const Z_INDEX = {
  /** Debug tools - always accessible (10000) */
  DEBUG_TOOLS: DEBUG_TOOLS_Z_INDEX,
  /** Priority tier bases - use modalController.push() for dynamic assignment */
  BACKGROUND: PRIORITY_BASE_Z[ModalPriority.BACKGROUND],
  NORMAL: PRIORITY_BASE_Z[ModalPriority.NORMAL],
  HIGH: PRIORITY_BASE_Z[ModalPriority.HIGH],
  BLOCKING: PRIORITY_BASE_Z[ModalPriority.BLOCKING],
} as const;
