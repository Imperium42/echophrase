<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  /**
   * Reusable Confirmation Modal
   *
   * Generic "Are you sure?" modal wrapping ModalWindow.
   * Compact variant: icon + title + message + two buttons.
   * Styled to match the delete confirmation in DataSection.
   */

  import ModalWindow from './ModalWindow.svelte';
  import { Icon } from '$lib/icons';
  import type { IconName } from '$lib/icons';

  type Props = {
    isOpen?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
    title?: string;
    message?: string;
    confirmText?: string;
    cancelText?: string;
    /** Optional icon shown above the title */
    icon?: IconName;
    /** Use danger styling (red) for the confirm button */
    danger?: boolean;
    /** Fade duration forwarded to ModalWindow. Pass 0 when stacking over
     * another modal so the dialog appears instantly. */
    fadeMs?: number;
  };

  let {
    isOpen = false,
    onConfirm,
    onCancel,
    title = 'Are you sure?',
    message = '',
    confirmText = 'Confirm',
    cancelText = 'Cancel',
    icon = undefined,
    danger = false,
    fadeMs = 0,
  }: Props = $props();
</script>

<ModalWindow {isOpen} {fadeMs} onClose={onCancel} hideCloseButton>
  <div class="confirm-modal">
    {#if icon}
      <div class="confirm-icon" class:danger>
        <Icon name={icon} size={32} strokeWidth={1.75} />
      </div>
    {/if}
    <h3 class="confirm-title">{title}</h3>
    {#if message}
      <p class="confirm-message">{message}</p>
    {/if}
    <div class="confirm-actions">
      <button class="confirm-cancel-btn" onclick={onCancel}>{cancelText}</button>
      <button class="confirm-proceed-btn" class:danger onclick={onConfirm}>{confirmText}</button>
    </div>
  </div>
</ModalWindow>

<style>
  .confirm-modal {
    text-align: center;
    padding: 0.5rem 0;
  }

  .confirm-icon {
    display: flex;
    justify-content: center;
    margin-bottom: 0.75rem;
    color: #a1a1aa;
  }
  .confirm-icon.danger {
    color: #f87171;
  }

  .confirm-title {
    font-size: 1.125rem;
    font-weight: 600;
    color: #e4e4e7;
    margin: 0 0 0.5rem;
  }

  .confirm-message {
    font-size: 0.8125rem;
    color: #a1a1aa;
    line-height: 1.5;
    margin: 0;
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 1.5rem;
  }

  .confirm-cancel-btn {
    background: rgba(63, 63, 70, 0.5);
    border: 1px solid rgba(82, 82, 91, 0.5);
    border-radius: 0.375rem;
    padding: 0.5rem 1rem;
    font-size: 0.8125rem;
    color: #a1a1aa;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .confirm-cancel-btn:hover {
    background: rgba(82, 82, 91, 0.6);
  }

  .confirm-proceed-btn {
    background: rgba(63, 63, 70, 0.6);
    border: 1px solid rgba(82, 82, 91, 0.6);
    border-radius: 0.375rem;
    padding: 0.5rem 1rem;
    font-size: 0.8125rem;
    font-weight: 500;
    color: #e4e4e7;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .confirm-proceed-btn:hover {
    background: rgba(82, 82, 91, 0.7);
  }

  .confirm-proceed-btn.danger {
    color: #fca5a5;
    border-color: rgba(239, 68, 68, 0.3);
    background: rgba(239, 68, 68, 0.1);
  }

  .confirm-proceed-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: rgba(239, 68, 68, 0.4);
  }

  /* Hide scrollbar on confirmation modal */
  :global(.modal-content:has(.confirm-modal)) {
    overflow: hidden;
  }
  :global(.modal-container:has(.confirm-modal)) {
    overflow: hidden;
    max-width: 24rem;
  }
</style>
