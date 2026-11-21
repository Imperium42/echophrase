#!/usr/bin/env bun

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * ensure-ort.mjs — One-shot ONNX Runtime bootstrap for Echophrase dev.
 *
 * Why this exists:
 *   `ort = "2.0.0-rc.12"` uses `load-dynamic` to load `libonnxruntime.{so,dylib,dll}`
 *   at runtime. Without a resolvable dylib, ORT falls back to CPU only — Parakeet
 *   TDT 0.6B on CPU takes 30–120s to preload, which presents as a hung "Loading
 *   model..." spinner on the record screen. This script makes dev onboarding
 *   brainless: it installs the correct ORT prebuilt into a repo-local
 *   `.ort/` directory (gitignored) and updates `env.yaml` to point at it.
 *
 * Idempotent: no-op if the library is already installed.
 * Runs automatically before `bun run tauri:dev` via the `predev` npm script.
 *
 * Production builds: NOT affected. `tauri.conf.json5` stages ORT resources into
 * the installer via its own build-time logic. This script is dev-only.
 */
import { mkdir, writeFile, readFile, access, unlink, copyFile } from 'node:fs/promises';
import { existsSync, createWriteStream, readFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const ORT_VERSION = '1.24.4';  // Latest stable 1.24.x as of 2026-04 (ort crate 2.0.0-rc.12 targets 1.24.x C API)
const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPT_DIR, '..');
const ORT_DIR = join(REPO_ROOT, '.ort');

// Per-platform install specs. Windows is intentionally omitted:
//   - Prod: DirectML runtime is bundled into the installer (~15MB).
//   - Dev:  Run `scripts/win/dev.ps1` which handles its own staging.
//
// Source is the Microsoft.ML.OnnxRuntime NuGet package (same source the release
// workflow uses), NOT github.com/microsoft/onnxruntime releases: that repo went
// 404 on 2026-11-20. NuGet ships the CPU build; Linux/WSL is CPU-only anyway.
const NUGET_URL = `https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime/${ORT_VERSION}`;
const PLATFORMS = {
  'linux-x64': {
    envKey: null, // Filled in at runtime - 'wsl' or 'linux'
    url: NUGET_URL,
    nupkgEntry: 'runtimes/linux-x64/native/libonnxruntime.so',
    libRelativePath: 'lib/libonnxruntime.so',
  },
  'darwin-arm64': {
    envKey: 'macos',
    url: NUGET_URL,
    nupkgEntry: 'runtimes/osx-arm64/native/libonnxruntime.dylib',
    libRelativePath: 'lib/libonnxruntime.dylib',
  },
};

/** Detect platform key + env.yaml section name in one pass. */
function detectPlatform() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === 'linux' && arch === 'x64') {
    // Split WSL vs native Linux — they have separate env.yaml sections.
    const isWsl = (() => {
      try { return readFileSync('/proc/version', 'utf8').toLowerCase().includes('microsoft'); }
      catch { return false; }
    })();
    return { key: 'linux-x64', envKey: isWsl ? 'wsl' : 'linux' };
  }
  if (platform === 'darwin' && arch === 'arm64') {
    return { key: 'darwin-arm64', envKey: 'macos' };
  }
  if (platform === 'win32') {
    return { key: null, reason: 'Windows dev uses scripts/win/dev.ps1 for ORT staging; no action needed here.' };
  }
  return { key: null, reason: `Unsupported ${platform}/${arch}.` };
}

async function exists(p) {
  try { await access(p); return true; } catch { return false; }
}

async function downloadToFile(url, dest) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`Download failed: HTTP ${res.status} for ${url}`);
  await pipeline(Readable.fromWeb(res.body), createWriteStream(dest));
}

/** Pull one entry out of a .nupkg (a zip) into `destFile`. Uses `unzip`, falling
 *  back to python3's zipfile when `unzip` is not installed. */
function extractNupkgEntry(archivePath, entry, destFile) {
  try {
    execSync(`unzip -p ${JSON.stringify(archivePath)} ${JSON.stringify(entry)} > ${JSON.stringify(destFile)}`, {
      stdio: ['ignore', 'ignore', 'inherit'],
      shell: '/bin/sh',
    });
  } catch {
    execSync(
      `python3 -I -c "import sys,zipfile;open(sys.argv[3],'wb').write(zipfile.ZipFile(sys.argv[1]).read(sys.argv[2]))" ` +
        `${JSON.stringify(archivePath)} ${JSON.stringify(entry)} ${JSON.stringify(destFile)}`,
      { stdio: 'inherit', shell: '/bin/sh' }
    );
  }
}

/**
 * Update (or insert) `ORT_DYLIB_PATH: "<absPath>"` under the matching platform
 * section in `env.yaml`. Creates the file from `env.yaml.template` if missing.
 * Does a line-level regex edit (no YAML parse — avoids pulling in a dep and
 * preserves all comments).
 */
async function updateEnvYaml(envKey, absLibPath) {
  const envPath = join(REPO_ROOT, 'env.yaml');
  const templatePath = join(REPO_ROOT, 'env.yaml.template');

  let content;
  if (await exists(envPath)) {
    content = await readFile(envPath, 'utf8');
  } else if (await exists(templatePath)) {
    console.log(`[ensure-ort]   env.yaml missing — seeding from env.yaml.template`);
    content = await readFile(templatePath, 'utf8');
  } else {
    throw new Error(`Neither env.yaml nor env.yaml.template found in ${REPO_ROOT}`);
  }

  // Line-based edit (no YAML parse — preserves comments). Done line-by-line rather
  // than with a single section regex because env.yaml may have mixed CRLF/LF line
  // endings: JS regex `.` doesn't match `\r`, so a section-spanning regex stalls at
  // the first blank CRLF line and an ORT_DYLIB_PATH below a comment block goes
  // unmatched — inserting a duplicate key (YAML "duplicated mapping key" error).
  const newLine = `  ORT_DYLIB_PATH: "${absLibPath}"`;
  const lines = content.split('\n');
  const headerIdx = lines.findIndex((l) => new RegExp(`^${envKey}:`).test(l));

  if (headerIdx === -1) {
    // Section didn't exist — append a new one.
    console.log(`[ensure-ort]   env.yaml has no '${envKey}:' section — appending`);
    content = content.trimEnd() + `\n\n${envKey}:\n${newLine}\n`;
    await writeFile(envPath, content);
    return;
  }

  // Section runs from headerIdx+1 until the next top-level key (a line starting
  // with a non-whitespace char). `\r` counts as whitespace, so blank CRLF lines
  // stay inside the section.
  let endIdx = headerIdx + 1;
  while (endIdx < lines.length && !/^\S/.test(lines[endIdx])) endIdx++;

  const ortLineRegex = /^\s+ORT_DYLIB_PATH\s*:/;
  let replaced = false;
  for (let i = headerIdx + 1; i < endIdx; i++) {
    if (ortLineRegex.test(lines[i])) {
      lines[i] = newLine;
      replaced = true;
      break;
    }
  }
  if (!replaced) lines.splice(headerIdx + 1, 0, newLine);

  await writeFile(envPath, lines.join('\n'));
}

/**
 * macOS only: stage the dylib into `src-tauri/macos-frameworks/libonnxruntime.dylib`.
 *
 * `tauri.conf.json5` declares `bundle.macOS.frameworks: ["./macos-frameworks/libonnxruntime.dylib"]`,
 * and `tauri_build::try_build` validates that path EXISTS at compile time — so a bare
 * `cargo build` / `bun run tauri:dev:mac` on a fresh clone fails with "Library not found"
 * unless the dylib is staged there first. CI stages it from its own download; this makes
 * local dev brainless too. Both the dir and `.ort/` are gitignored, so nothing is committed.
 */
async function stageMacosFramework(libPath) {
  const frameworksDir = join(REPO_ROOT, 'src-tauri', 'macos-frameworks');
  const dest = join(frameworksDir, 'libonnxruntime.dylib');
  await mkdir(frameworksDir, { recursive: true });
  await copyFile(libPath, dest);
  console.log(`[ensure-ort] ✓ Staged dylib for macOS bundler: ${dest}`);
}

async function main() {
  const { key, envKey, reason } = detectPlatform();
  if (!key) {
    console.log(`[ensure-ort] Skipping: ${reason}`);
    return;
  }
  const spec = PLATFORMS[key];
  spec.envKey ??= envKey;

  const installDir = join(ORT_DIR, key);
  const libPath = join(installDir, spec.libRelativePath);

  if (await exists(libPath)) {
    console.log(`[ensure-ort] ✓ ORT ${ORT_VERSION} already installed (${libPath})`);
    await updateEnvYaml(spec.envKey, libPath);
    console.log(`[ensure-ort] ✓ env.yaml [${spec.envKey}] ORT_DYLIB_PATH is up to date`);
    if (key === 'darwin-arm64') await stageMacosFramework(libPath);
    return;
  }

  console.log(`[ensure-ort] Installing ONNX Runtime ${ORT_VERSION} for ${key}…`);
  await mkdir(installDir, { recursive: true });

  const archivePath = join(installDir, 'archive.nupkg');
  console.log(`[ensure-ort]   Downloading ${spec.url}`);
  await downloadToFile(spec.url, archivePath);

  console.log(`[ensure-ort]   Extracting…`);
  await mkdir(dirname(libPath), { recursive: true });
  extractNupkgEntry(archivePath, spec.nupkgEntry, libPath);
  await unlink(archivePath).catch(() => {});

  if (!await exists(libPath)) {
    throw new Error(
      `Expected ${libPath} after extraction but not found. ` +
      `The archive layout may have changed.`
    );
  }

  await updateEnvYaml(spec.envKey, libPath);
  console.log(`[ensure-ort] ✓ Installed: ${libPath}`);
  console.log(`[ensure-ort] ✓ env.yaml [${spec.envKey}] ORT_DYLIB_PATH updated`);
  if (key === 'darwin-arm64') await stageMacosFramework(libPath);
  console.log(`[ensure-ort]   Parakeet TDT 0.6B preload should now complete in ~3s (was ~120s on CPU).`);
}

main().catch((e) => {
  console.error(`[ensure-ort] FAILED: ${e instanceof Error ? e.message : e}`);
  console.error(`[ensure-ort]   You can fall back to manual setup — see env.yaml.template.`);
  process.exit(1);
});
