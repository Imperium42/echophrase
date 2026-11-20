#!/usr/bin/env node

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Bump version in tauri.conf.json5
 * Usage: node scripts/bump-version.js 0.8.1
 */

import { readFileSync, writeFileSync } from 'fs';

const version = process.argv[2];
if (!version) {
  console.error('Usage: node scripts/bump-version.js <version>');
  process.exit(1);
}

const tauriConfigPath = 'src-tauri/tauri.conf.json5';
let content = readFileSync(tauriConfigPath, 'utf8');

// Replace version in json5 (handles both quoted formats)
content = content.replace(
  /"version":\s*"[^"]*"/,
  `"version": "${version}"`
);

writeFileSync(tauriConfigPath, content);
console.log(`Updated ${tauriConfigPath} to version ${version}`);
