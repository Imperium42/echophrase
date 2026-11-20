// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import js from '@eslint/js';

export default [
  {
    ignores: [
      '**/*.ts',
      '**/*.svelte',
      'dist/**',
      'build/**',
      '.svelte-kit/**',
      'node_modules/**',
      'src-tauri/**'
    ]
  },
  js.configs.recommended,
  {
    files: ['**/*.js', '**/*.cjs', '**/*.mjs'],
    languageOptions: {
      ecmaVersion: 2020,
      sourceType: 'module',
      globals: {
        browser: true,
        node: true,
        es2017: true,
        window: true,
        document: true,
        console: true,
        process: true,
        Buffer: true,
        setTimeout: true,
        clearTimeout: true,
        setInterval: true,
        clearInterval: true,
        global: true,
      }
    },
    rules: {
      'no-unused-vars': ['error', { argsIgnorePattern: '^_' }],
    },
  }
];