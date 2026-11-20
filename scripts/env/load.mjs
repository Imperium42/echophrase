#!/usr/bin/env bun

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

// Load environment variables from env.yaml based on platform.
// Imports schema for structural validation before applying any values.

import { readFileSync, writeFileSync, existsSync, copyFileSync } from 'fs';
import { load } from 'js-yaml';
import { homedir } from 'os';
import { EnvConfigSchema } from './schema.ts';

// Detect platform
export function detectPlatform() {
  if (process.platform === 'win32') {
    return 'windows';
  }

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

  if (process.platform === 'darwin') {
    return 'macos';
  }

  return 'unknown';
}

// Detect CPU architecture
export function detectArchitecture() {
  const arch = process.arch;
  return arch === 'arm64' ? 'arm64' : 'x64';
}

// Expand environment variables in a string value
export function expandVars(value) {
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

// Dev server ports. DEV_PORT (default 8420) is the single source of truth: the Vite
// dev server binds it, HMR binds DEV_PORT + 1, and vite.config.js / lib.rs read the
// same env var. Windows Hyper-V randomly reserves port ranges, so a machine that
// can't bind 8420 sets DEV_PORT in its (gitignored) env.yaml instead of editing tracked files.
// strictPort is on, so a taken port would make Vite die and Tauri hang forever
// waiting on devUrl. We preflight-bind these and fail loud instead.
const DEFAULT_DEV_PORT = 8420;
/** Resolve + validate DEV_PORT. Exits with a clear error on nonsense values. */
export function resolveDevPort() {
  const raw = process.env.DEV_PORT;
  if (raw === undefined || raw === '') return DEFAULT_DEV_PORT;
  const port = Number(raw);
  // HMR uses port + 1, so 65535 is out; stay clear of the OS ephemeral range too.
  if (!Number.isInteger(port) || port < 1024 || port > 49150) {
    console.error(`\nInvalid DEV_PORT "${raw}": use an integer between 1024 and 49150.`);
    process.exit(1);
  }
  return port;
}

// Must be called AFTER loadEnv(): DEV_PORT usually comes from env.yaml, which loadEnv applies
// to process.env - resolving it at import time would silently ignore the file.
function devPortsFor(devPort) {
  return [
    { port: devPort, label: 'Vite dev server' },
    { port: devPort + 1, label: 'Vite HMR websocket' }
  ];
}

// tauri.conf.json5 hard-codes the default port (devUrl + the CSP ws:// entries). For any other
// DEV_PORT, write a JSON override that `tauri dev --config` merges over it. Returns the file
// path, or null when the default port is in use (no override needed).
function writeTauriPortOverride(devPort) {
  if (devPort === DEFAULT_DEV_PORT) return null;
  const conf = readFileSync('src-tauri/tauri.conf.json5', 'utf8');
  const m = conf.match(/"csp":\s*"((?:[^"\\]|\\.)*)"/);
  if (!m) {
    console.error('Could not find app.security.csp in src-tauri/tauri.conf.json5 to rewrite DEV_PORT.');
    process.exit(1);
  }
  const csp = JSON.parse(`"${m[1]}"`)
    .replaceAll(`localhost:${DEFAULT_DEV_PORT + 1}`, `localhost:${devPort + 1}`)
    .replaceAll(`localhost:${DEFAULT_DEV_PORT}`, `localhost:${devPort}`);
  const file = '.tauri-dev-port.json';
  writeFileSync(
    file,
    JSON.stringify({
      build: { devUrl: `http://localhost:${devPort}` },
      app: { security: { csp } }
    })
  );
  return file;
}

// Try to bind a TCP port on loopback. Resolves true if free, false if in use.
// Under WSL2 mirrored networking a Windows-side listener also collides here,
// which is exactly the cross-instance conflict we want to catch.
async function isPortFree(port) {
  const net = await import('net');
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once('error', () => resolve(false));
    server.once('listening', () => server.close(() => resolve(true)));
    server.listen(port, '127.0.0.1');
  });
}

// Best-effort: name the process holding a port so the error is actionable.
async function whoHoldsPort(port) {
  try {
    const { execSync } = await import('child_process');
    // ss is present on Linux/WSL; ignore failures (e.g. macOS uses lsof).
    const out = execSync(`ss -ltnp 2>/dev/null | grep ':${port} ' || lsof -nP -iTCP:${port} -sTCP:LISTEN 2>/dev/null || true`, {
      encoding: 'utf8',
      timeout: 3000
    }).trim();
    return out || null;
  } catch {
    return null;
  }
}

// Preflight the dev ports. On conflict, print a descriptive error and exit 1
// so the launch fails immediately rather than hanging on a dead Vite server.
async function ensureDevPortsFree(devPort) {
  const conflicts = [];
  for (const { port, label } of devPortsFor(devPort)) {
    if (!(await isPortFree(port))) {
      conflicts.push({ port, label });
    }
  }

  if (conflicts.length === 0) return;

  console.error('\nDev port conflict - cannot start.');
  for (const { port, label } of conflicts) {
    console.error(`   Port ${port} (${label}) is already in use.`);
    const holder = await whoHoldsPort(port);
    if (holder) {
      console.error(`     -> ${holder.split('\n')[0]}`);
    }
  }
  console.error('\nLikely cause: another Echophrase dev instance is already running');
  console.error('(a Windows-native build collides with WSL via mirrored networking).');
  console.error('\nFix:');
  console.error('   - Close the other running app/dev server, or');
  console.error('   - Kill stray servers: bun run scripts/kill-all-servers.sh, or');
  console.error('   - Probe free ports: python scripts/check-ports.py');
  process.exit(1);
}

// Load env.yaml and apply values to process.env for the given platform.
// Returns true on success, false on failure.
export async function loadEnv(platformOverride = null) {
  const platform = platformOverride || detectPlatform();
  const envFile = 'env.yaml';
  const templateFile = 'env.yaml.template';

  if (!existsSync(envFile)) {
    if (existsSync(templateFile)) {
      console.log('Creating env.yaml from template...');
      copyFileSync(templateFile, envFile);
    } else {
      console.error('No env.yaml or env.yaml.template found!');
      process.exit(1);
    }
  }

  console.log(`Loading environment for platform: ${platform}`);

  try {
    const config = load(readFileSync(envFile, 'utf8'));

    const validation = EnvConfigSchema.safeParse(config);
    if (!validation.success) {
      console.error('\nInvalid env.yaml structure:');
      validation.error.issues.forEach(issue => {
        console.error(`   ${issue.path.join('.')}: ${issue.message}`);
      });
      console.error('\nSuggestions:');
      console.error('   - Check env.yaml.template for required keys');
      console.error('   - Run: bun run env:validate');
      console.error('   - Validate YAML syntax: https://www.yamllint.com/');
      process.exit(1);
    }

    // Apply shared variables first
    console.log('Loading shared configuration...');
    if (config.shared) {
      for (const [key, value] of Object.entries(config.shared)) {
        if (value !== null && value !== undefined) {
          process.env[key] = expandVars(String(value));
          console.log(`   + ${key}`);
        }
      }
    }

    // Apply platform-specific variables (override shared)
    console.log(`Loading ${platform}-specific configuration...`);
    if (config[platform]) {
      for (const [key, value] of Object.entries(config[platform])) {
        if (value !== null && value !== undefined) {
          process.env[key] = expandVars(String(value));
          console.log(`   + ${key}`);
        }
      }
    }

    // macOS: auto-detect architecture and set ONNX Runtime path
    if (platform === 'macos' && config.macos) {
      const arch = detectArchitecture();
      const ortKey = `ORT_DYLIB_PATH_${arch.toUpperCase()}`;
      const ortPath = config.macos[ortKey];

      if (ortPath) {
        const expanded = expandVars(ortPath);
        process.env.ORT_DYLIB_PATH = expanded;
        console.log(`Detected macOS ${arch} - using ${ortKey}`);

        if (!existsSync(expanded)) {
          console.error(`\nONNX Runtime library not found for ${arch}`);
          console.error(`   Expected: ${expanded}`);
          console.error(`\nDownload ONNX Runtime:`);
          console.error(`   bun run setup:ort`);
          console.error(`   Or manually: Microsoft.ML.OnnxRuntime 1.24.4 from https://www.nuget.org/packages/Microsoft.ML.OnnxRuntime/1.24.4`);
          process.exit(1);
        }
      } else {
        console.warn(`Warning: ${ortKey} not found in env.yaml - falling back to manual ORT_DYLIB_PATH if set`);
      }
    }

    process.env.ECHOPHRASE_PLATFORM = platform;

    console.log('\nEnvironment loaded successfully!');
    console.log('\nKey variables set:');
    console.log(`   CUDA_HOME: ${process.env.CUDA_HOME || 'not set'}`);
    console.log(`   CT2_CUDA_TRUE_FP16_GEMM: ${process.env.CT2_CUDA_TRUE_FP16_GEMM || 'not set'}`);
    if (platform === 'wsl') {
      console.log(`   PULSE_SERVER: ${process.env.PULSE_SERVER || 'not set'}`);
    }

    return true;
  } catch (error) {
    console.error('Error loading environment:', error.message);
    return false;
  }
}

// Entry point: load env then optionally spawn a command
if (import.meta.main) {
  const args = process.argv.slice(2);

  let platform = null;
  let commandToRun = args;

  if (['wsl', 'windows', 'linux', 'macos'].includes(args[0])) {
    platform = args[0];
    commandToRun = args.slice(1);
  } else if (args[0] === '--') {
    commandToRun = args.slice(1);
  }

  if (commandToRun[0] === '--staging') {
    process.env.VITE_APP_ENV = 'staging';
    console.log('Mode: staging');
    commandToRun = commandToRun.slice(1);
  } else if (commandToRun[0] === '--production') {
    process.env.VITE_APP_ENV = 'production';
    console.log('Mode: production');
    commandToRun = commandToRun.slice(1);
  }

  loadEnv(platform).then(async success => {
    if (success && commandToRun.length > 0) {
      // Only guard the dev server (which binds the fixed Vite/HMR ports).
      // `tauri build` and other commands don't bind these, so skip the check.
      if (commandToRun.includes('dev')) {
        const devPort = resolveDevPort();
        await ensureDevPortsFree(devPort);
        process.env.DEV_PORT = String(devPort);
        const override = writeTauriPortOverride(devPort);
        const at = commandToRun.indexOf('dev');
        if (override && commandToRun[at - 1] === 'tauri') {
          commandToRun.splice(at + 1, 0, '--config', override);
          console.log(`DEV_PORT=${devPort}: merging ${override} over tauri.conf.json5`);
        }
      }

      const { spawn } = await import('child_process');
      const [command, ...commandArgs] = commandToRun;

      const child = spawn(command, commandArgs, {
        stdio: 'inherit',
        env: process.env,
        shell: true
      });

      child.on('exit', (code) => {
        process.exit(code || 0);
      });
    }
  });
}
