// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

import { defineConfig } from 'eslint/config';
import js from '@eslint/js';
import ts from 'typescript-eslint';
import svelte from 'eslint-plugin-svelte';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import svelteConfig from './svelte.config.js';

export default defineConfig(
  {
    ignores: [
      'dist/**',
      'build/**',
      '.svelte-kit/**',
      'node_modules/**',
      'src-tauri/**',
      'coverage/**'
    ]
  },
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs.recommended,
  prettier,
  ...svelte.configs.prettier,
  {
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.node }
    },
    rules: {
      // TypeScript already reports undefined names, and no-undef misfires on type-only globals.
      'no-undef': 'off',
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrorsIgnorePattern: '^_' }
      ],
      // Every href in this Tauri desktop app is an external URL (opened via launchUrl) or a
      // generic <a>/<button> primitive; there is no SvelteKit base path to resolve() against.
      'svelte/no-navigation-without-resolve': 'off'
    }
  },
  {
    // Tests mock Tauri/window internals, where `any` is the pragmatic type.
    files: ['**/*.test.ts', '**/*.test.js'],
    rules: { '@typescript-eslint/no-explicit-any': 'off' }
  },
  {
    files: ['**/*.svelte', '**/*.svelte.ts', '**/*.svelte.js'],
    languageOptions: {
      parserOptions: {
        extraFileExtensions: ['.svelte'],
        parser: ts.parser,
        svelteConfig
      }
    }
  }
);
