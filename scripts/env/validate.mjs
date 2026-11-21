#!/usr/bin/env bun

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Validate env.yaml structure and paths.
 * Run: bun scripts/env/validate.mjs [--check-paths]
 *
 * Flags:
 *   --check-paths  Also verify that platform-specific model directories exist
 */

import { readFileSync, existsSync } from 'fs';
import { load } from 'js-yaml';
import { homedir } from 'os';
import { EnvConfigSchema } from './schema.ts';

// Expand environment variables in a string value
function expandVars(value) {
  if (typeof value !== 'string') return value;

  return value
    .replace(/\$\{([^}]+)\}/g, (_, name) => {
      if (name === 'HOME') return homedir();
      if (name === 'USERNAME') return process.env.USERNAME || process.env.USER || 'user';
      if (name === 'USERPROFILE') return process.env.USERPROFILE || homedir();
      return process.env[name] || '';
    })
    .replace(/\$([A-Z_][A-Z0-9_]*)/g, (_, name) => {
      if (name === 'HOME') return homedir();
      return process.env[name] || '';
    });
}

// Detect platform
function detectPlatform() {
  if (process.platform === 'win32') return 'windows';
  if (process.platform === 'darwin') return 'macos';

  if (process.platform === 'linux') {
    try {
      const procVersion = readFileSync('/proc/version', 'utf8');
      if (procVersion.toLowerCase().includes('microsoft') || procVersion.toLowerCase().includes('wsl')) {
        return 'wsl';
      }
    } catch {}

    if (process.env.WSL_DISTRO_NAME || process.env.WSL_INTEROP) {
      return 'wsl';
    }

    return 'linux';
  }

  return 'unknown';
}

async function validateEnv(checkPaths = false) {
  const envFile = 'env.yaml';
  let exitCode = 0;

  console.log('Validating env.yaml...\n');

  if (!existsSync(envFile)) {
    console.error('env.yaml not found!');
    console.error('   Create it from template: cp env.yaml.template env.yaml\n');
    process.exit(1);
  }

  try {
    const config = load(readFileSync(envFile, 'utf8'));

    console.log('Validating schema...');
    const validation = EnvConfigSchema.safeParse(config);

    if (!validation.success) {
      console.error('Schema validation failed:\n');
      validation.error.issues.forEach(issue => {
        const path = issue.path.join('.');
        console.error(`   ${path}: ${issue.message}`);
      });
      console.error('\nSuggestions:');
      console.error('   - Check env.yaml.template for required keys');
      console.error('   - Validate YAML syntax: https://www.yamllint.com/');
      exitCode = 1;
    } else {
      console.log('Schema is valid\n');
    }

    // Check platform-specific paths if requested. ORT_DYLIB_PATH is required:
    // ort uses load-dynamic to load onnxruntime at runtime.
    if (checkPaths) {
      const platform = detectPlatform();
      console.log(`Checking ${platform}-specific paths...\n`);

      const ortPath = config[platform]?.ORT_DYLIB_PATH;
      if (ortPath) {
        const expanded = expandVars(ortPath);
        console.log(`   Checking ORT_DYLIB_PATH: ${expanded}`);
        if (!existsSync(expanded)) {
          console.warn('   Warning: not found - run `bun run setup:ort`');
        } else {
          console.log('   Found');
        }
      }
    }

    if (exitCode === 0) {
      console.log('All validations passed!');
    }

    process.exit(exitCode);
  } catch (error) {
    console.error('Error reading env.yaml:', error.message);
    process.exit(1);
  }
}

const checkPaths = process.argv.includes('--check-paths');
validateEnv(checkPaths);
