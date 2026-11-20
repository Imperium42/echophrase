// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Settings Module Index
 *
 * Re-exports all settings components for convenient imports.
 *
 * Usage:
 * ```typescript
 * import { SettingsModal } from '$lib/components/settings';
 * // or for granular imports:
 * import { SettingsSection, SettingToggle } from '$lib/components/settings/primitives';
 * ```
 */

// Main modal component
export { default as SettingsModal } from './SettingsModal.svelte';

// Header and footer
export { default as SettingsHeader } from './SettingsHeader.svelte';
export { default as SettingsFooter } from './SettingsFooter.svelte';

// Settings context
export { setSettingsContext, getSettingsContext } from './context';
export type { SettingsContext } from './context';

// Primitives (re-export from primitives module)
export * from './primitives';

// Sections (re-export from sections module)
export * from './sections';
