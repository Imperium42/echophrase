// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Public icon API. All consumers import from `$lib/icons` only.
 *
 * - `Icon`: the single component used at every call site.
 * - `IconName`: the union of valid `name` props (autocompleted, type-checked).
 *
 * Adding a new icon: edit `./registry.ts` and add a row to `ICONS`.
 */
export { default as Icon } from './Icon.svelte';
export type { IconName } from './registry';
