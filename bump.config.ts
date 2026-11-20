// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { defineConfig } from 'bumpp'
import tauri from 'tauri-version'

export default defineConfig({
  all: true,           // Commit, tag, and push
  execute: tauri(),    // Sync Cargo.toml and tauri.conf.json5
})
