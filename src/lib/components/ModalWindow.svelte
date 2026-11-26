<!--
SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC

SPDX-License-Identifier: MPL-2.0
-->

<script lang="ts">
  import { fade } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { onDestroy, tick } from 'svelte';
  import ModalCloseButton from './ModalCloseButton.svelte';
  import { modalController } from '$lib/stores/modalController';

  export let isOpen = false;
  export let maxWidth = 'max-w-2xl';
  export let onClose: (() => void) | undefined = undefined;
  /** Hide the built-in close button (use when content has its own) */
  export let hideCloseButton = false;
  /** Optional accessible label for the dialog (id of an element). */
  export let ariaLabelledBy: string | undefined = undefined;
  /** Override z-index for modals that must stack above other modals (e.g. update modal over settings). Default: 50. */
  export let zIndex: number = 50;
  /** Fade duration in ms. Pass 0 for modals that take over from another
   * modal in the same frame - a fade there reads as a backdrop pulse. */
  export let fadeMs: number = 90;

  let containerEl: HTMLDivElement | null = null;
  let lastFocusedBeforeOpen: HTMLElement | null = null;

  // Focusable selector used by the focus trap. Excludes elements with
  // negative tabindex and disabled controls so Tab cycles only what the
  // user can actually interact with.
  const FOCUSABLE_SELECTOR =
    'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), [tabindex]:not([tabindex="-1"])';

  function focusableElements(): HTMLElement[] {
    if (!containerEl) return [];
    return Array.from(containerEl.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget && onClose) {
      onClose();
    }
  }

  // ESC is handled by modalController (the single source of truth): while open,
  // this window is a layer there, so ESC closes only the topmost modal - a
  // dialog opened from Settings closes before Settings itself.
  let layerId: string | null = null;
  $: syncLayer(isOpen);
  function syncLayer(open: boolean) {
    if (open && !layerId) {
      layerId = modalController.push({
        onClose: () => onClose?.(),
        allowEsc: onClose !== undefined,
      });
    } else if (!open && layerId) {
      modalController.pop(layerId);
      layerId = null;
    }
  }
  const topModalId = modalController.topModalId;

  function handleKeydown(e: KeyboardEvent) {
    // Only the topmost modal traps Tab, or Settings would fight a dialog above it.
    if (!isOpen || (layerId !== null && $topModalId !== layerId)) return;
    if (e.key === 'Tab') {
      const els = focusableElements();
      if (els.length === 0) {
        e.preventDefault();
        containerEl?.focus();
        return;
      }
      const first = els[0];
      const last = els[els.length - 1];
      const active = document.activeElement as HTMLElement | null;
      if (e.shiftKey) {
        if (active === first || !containerEl?.contains(active)) {
          e.preventDefault();
          last.focus();
        }
      } else if (active === last) {
        e.preventDefault();
        first.focus();
      }
    }
  }

  // Open/close lifecycle: capture previously-focused element, move focus
  // into the dialog after mount, restore focus on close.
  $: if (isOpen) {
    if (typeof document !== 'undefined') {
      lastFocusedBeforeOpen = document.activeElement as HTMLElement | null;
    }
    tick().then(() => {
      if (!isOpen || !containerEl) return;
      const els = focusableElements();
      (els[0] ?? containerEl).focus();
    });
  } else if (lastFocusedBeforeOpen) {
    lastFocusedBeforeOpen.focus?.();
    lastFocusedBeforeOpen = null;
  }

  onDestroy(() => {
    if (layerId) modalController.pop(layerId);
    if (lastFocusedBeforeOpen) {
      lastFocusedBeforeOpen.focus?.();
      lastFocusedBeforeOpen = null;
    }
  });
</script>

<svelte:window on:keydown={handleKeydown} />

{#if isOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="modal-backdrop"
    style="--modal-z-index: {zIndex}"
    transition:fade={{ duration: fadeMs, easing: cubicOut }}
    onclick={handleBackdropClick}
  >
    <div
      bind:this={containerEl}
      role="dialog"
      aria-modal="true"
      aria-labelledby={ariaLabelledBy}
      tabindex="-1"
      class="modal-container {maxWidth}"
      transition:fade={{ duration: fadeMs, easing: cubicOut }}
    >
      {#if onClose && !hideCloseButton}
        <ModalCloseButton {onClose} />
      {/if}

      <div class="modal-content">
        <slot />
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    /* Solid dark background instead of expensive backdrop-filter blur */
    background: rgba(0, 0, 0, 0.85);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--modal-z-index, 50);
    /* Isolate to prevent repaints affecting main UI */
    contain: layout style paint;
    isolation: isolate;
  }

  .modal-container {
    position: relative;
    background: linear-gradient(180deg, rgba(24, 24, 27, 0.98) 0%, rgba(24, 24, 27, 0.95) 100%);
    /* Removed backdrop-filter - backdrop already has blur, double blur causes glitches */
    border-radius: 1rem;
    border: 1px solid rgba(255, 255, 255, 0.06);
    width: 100%;
    max-width: 32rem;
    margin: 1rem;
    /* Viewport units are physical - divide out the UI-scale transform */
    max-height: calc(90vh / var(--ui-zoom, 1));
    display: flex;
    flex-direction: column;
    box-shadow:
      0 25px 50px -12px rgba(0, 0, 0, 0.5),
      0 0 0 1px rgba(255, 255, 255, 0.03);
    outline: none;
  }

  .modal-container.max-w-2xl {
    max-width: 32rem;
  }

  /* Settings: grow with the window instead of leaving dead space at large
     window sizes, capped so text lines stay readable. */
  .modal-container.max-w-settings {
    max-width: min(92%, 52rem);
  }

  .modal-content {
    padding: 1.5rem 2rem;
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0; /* Allow flex children to shrink */
    /* Short windows: scroll inside the modal (padding included) instead of clipping */
    overflow-y: auto;
  }
</style>
