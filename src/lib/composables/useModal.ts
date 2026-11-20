// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * useModal - thin composable for registering/deregistering with modalController
 *
 * Usage:
 * ```typescript
 * const modal = useModal('settings', { priority: ModalPriority.NORMAL });
 *
 * // When modal opens:
 * modal.open(() => { visible = false; });
 *
 * // When modal closes:
 * modal.close();
 *
 * // Auto-cleanup on component destroy:
 * onDestroy(() => modal.close());
 * ```
 */

import { modalController, ModalPriority } from '$lib/stores/modalController';

export interface UseModalOptions {
  priority?: ModalPriority;
  allowEsc?: boolean;
}

export function useModal(id: string, options: UseModalOptions = {}) {
  const { priority = ModalPriority.NORMAL, allowEsc = true } = options;
  let registeredId: string | null = null;

  return {
    /** Register this modal as open in the stack */
    open(onClose: () => void) {
      // Prevent double-registration
      if (registeredId) return;
      registeredId = modalController.push({ id, onClose, priority, allowEsc });
    },

    /** Remove this modal from the stack */
    close() {
      if (registeredId) {
        modalController.pop(registeredId);
        registeredId = null;
      }
    },

    /** Whether this modal is currently registered */
    get isOpen() {
      return registeredId !== null;
    },

    /** The modal ID */
    id,
  };
}

export { ModalPriority };
