#!/usr/bin/env bun

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * ensure-ort.mjs - the single ONNX Runtime stager (dev, local builds and release CI).
 * Run from the repo root: `bun scripts/ensure-ort.mjs [--ci]`.
 *
 * Why this exists:
 *   `ort = "2.0.0-rc.12"` uses `load-dynamic` to load `onnxruntime.{dll,so,dylib}` at
 *   runtime. Without one, nothing transcribes. This script downloads the pinned,
 *   SHA256-verified NuGet packages below and stages the natives where each platform
 *   looks for them, so dev, `tauri build` and release CI all ship the same runtime.
 *
 * Where it stages:
 *   - Windows: `src-tauri/resources/ort/directml/` (onnxruntime.dll, onnxruntime_providers_shared.dll,
 *     DirectML.dll). That is the bundled-resource path `select_ort_runtime()` probes in production,
 *     and Tauri copies resources next to the dev exe too, so dev needs no `ORT_DYLIB_PATH` at all.
 *     A placeholder or non-existent `ORT_DYLIB_PATH` in env.yaml's `windows:` section is removed
 *     (it would win over the bundled copy); a real existing path is left alone as a manual override.
 *   - Linux/WSL/macOS: `.ort/<platform>/lib/`, and `ORT_DYLIB_PATH` in env.yaml points at it.
 *     macOS also copies the dylib to `src-tauri/macos-frameworks/` (see stageMacosFramework).
 *
 * `--ci`: stage only, never touch env.yaml (release workflows).
 * Idempotent: if every staged file still matches the hashes recorded at staging time, no-op.
 */
/* global fetch */
import { mkdir, writeFile, readFile, rm, copyFile } from 'node:fs/promises';
import { existsSync, createWriteStream, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, resolve, dirname, basename } from 'node:path';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// ---------------------------------------------------------------------------
// Manifest: the ONE place ORT versions and hashes live. Bump version + sha256 together.
// ORT stays on 1.24.x: ort 2.0.0-rc.12 targets the 1.24 C API.
// DirectML version = the one the ORT DirectML nuspec depends on.
// ---------------------------------------------------------------------------
const ORT_VERSION = '1.24.4';
const PACKAGES = {
  ortCpu: {
    id: 'Microsoft.ML.OnnxRuntime',
    version: ORT_VERSION,
    sha256: '4b978d5065b85e7004b6c6f60ca494bd978fbe6836cbf0a0b52d82b61ab99638',
  },
  ortDirectml: {
    id: 'Microsoft.ML.OnnxRuntime.DirectML',
    version: ORT_VERSION,
    sha256: '57e9f11b73437bef7a309496135d4c1f96b1a8e9ddba60013fa27bfc1d788681',
  },
  directml: {
    id: 'Microsoft.AI.DirectML',
    version: '1.15.4',
    sha256: '4e7cb7ddce8cf837a7a75dc029209b520ca0101470fcdf275c1f49736a3615b9',
  },
};

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(SCRIPT_DIR, '..');
const ORT_DIR = join(REPO_ROOT, '.ort');
const DIRECTML_DEST = join(REPO_ROOT, 'src-tauri', 'resources', 'ort', 'directml');

/** Per target: which package entries go where, and the library ORT_DYLIB_PATH should name. */
function windowsTarget(key, rid, dmlBin) {
  return {
    key,
    destDir: DIRECTML_DEST,
    writesEnvPath: false,
    files: [
      { pkg: 'ortDirectml', entry: `runtimes/${rid}/native/onnxruntime.dll` },
      { pkg: 'ortDirectml', entry: `runtimes/${rid}/native/onnxruntime_providers_shared.dll` },
      { pkg: 'directml', entry: `bin/${dmlBin}/DirectML.dll` },
    ],
  };
}
function unixTarget(key, rid, libName) {
  return {
    key,
    destDir: join(ORT_DIR, key, 'lib'),
    writesEnvPath: true,
    lib: libName,
    files: [{ pkg: 'ortCpu', entry: `runtimes/${rid}/native/${libName}` }],
  };
}
const TARGETS = {
  'win32-x64': windowsTarget('win32-x64', 'win-x64', 'x64-win'),
  'win32-arm64': windowsTarget('win32-arm64', 'win-arm64', 'arm64-win'),
  'linux-x64': unixTarget('linux-x64', 'linux-x64', 'libonnxruntime.so'),
  'linux-arm64': unixTarget('linux-arm64', 'linux-arm64', 'libonnxruntime.so'),
  'darwin-arm64': unixTarget('darwin-arm64', 'osx-arm64', 'libonnxruntime.dylib'),
};

const log = (msg) => console.log(`[ensure-ort] ${msg}`);

function detectEnvKey() {
  if (process.platform === 'win32') return 'windows';
  if (process.platform === 'darwin') return 'macos';
  try {
    return readFileSync('/proc/version', 'utf8').toLowerCase().includes('microsoft') ? 'wsl' : 'linux';
  } catch {
    return 'linux';
  }
}

async function sha256File(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex');
}

async function downloadToFile(url, dest) {
  const res = await fetch(url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`Download failed: HTTP ${res.status} for ${url}`);
  await pipeline(Readable.fromWeb(res.body), createWriteStream(dest));
}

/** Extract `entries` of a .nupkg (a zip) into `outDir`, keeping their relative paths.
 *  Windows: System32 bsdtar (ships with Windows 10+; Git Bash's GNU tar can't read zips).
 *  Elsewhere: unzip, falling back to python3's zipfile. No shell, so no quoting issues. */
function extractNupkgEntries(archivePath, entries, outDir) {
  if (process.platform === 'win32') {
    const tar = join(process.env.SystemRoot ?? 'C:/Windows', 'System32', 'tar.exe');
    execFileSync(tar, ['-xf', archivePath, '-C', outDir, ...entries], { stdio: 'inherit' });
    return;
  }
  try {
    execFileSync('unzip', ['-o', '-q', archivePath, ...entries, '-d', outDir], { stdio: 'inherit' });
  } catch {
    execFileSync('python3', [
      '-I', '-c',
      'import sys,zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2], sys.argv[3:])',
      archivePath, outDir, ...entries,
    ], { stdio: 'inherit' });
  }
}

/** Stamp = manifest fingerprint + hash of every staged file. Fast path compares against it.
 *  Lives in .ort/, never in destDir: the Windows dest is globbed into the installer. */
function stampPath(target) {
  return join(ORT_DIR, 'stamps', `${target.key}.json`);
}
function manifestFingerprint(target) {
  return target.files.map((f) => `${PACKAGES[f.pkg].id}@${PACKAGES[f.pkg].version}:${f.entry}`).join('|');
}
async function isUpToDate(target) {
  const path = stampPath(target);
  if (!existsSync(path)) return false;
  try {
    const stamp = JSON.parse(await readFile(path, 'utf8'));
    if (stamp.manifest !== manifestFingerprint(target)) return false;
    for (const [name, hash] of Object.entries(stamp.files)) {
      const file = join(target.destDir, name);
      if (!existsSync(file) || (await sha256File(file)) !== hash) return false;
    }
    return true;
  } catch {
    return false;
  }
}

/** Download + verify every package first, then extract into a temp dir, then copy into
 *  place. A hash mismatch aborts before anything touches destDir (no partial staging). */
async function stage(target) {
  const tmp = join(ORT_DIR, 'tmp');
  await rm(tmp, { recursive: true, force: true });
  await mkdir(tmp, { recursive: true });
  try {
    const pkgKeys = [...new Set(target.files.map((f) => f.pkg))];
    const archives = {};
    for (const key of pkgKeys) {
      const pkg = PACKAGES[key];
      const url = `https://www.nuget.org/api/v2/package/${pkg.id}/${pkg.version}`;
      const archive = join(tmp, `${key}.nupkg`);
      log(`  Downloading ${pkg.id} ${pkg.version}`);
      await downloadToFile(url, archive);
      const got = await sha256File(archive);
      if (got !== pkg.sha256) {
        throw new Error(`SHA256 mismatch for ${pkg.id} ${pkg.version}: expected ${pkg.sha256}, got ${got}. Nothing was staged.`);
      }
      archives[key] = archive;
    }

    const extracted = [];
    for (const key of pkgKeys) {
      const out = join(tmp, `${key}-x`);
      await mkdir(out, { recursive: true });
      const entries = target.files.filter((f) => f.pkg === key).map((f) => f.entry);
      extractNupkgEntries(archives[key], entries, out);
      for (const entry of entries) {
        const file = join(out, entry);
        if (!existsSync(file)) throw new Error(`${entry} missing from ${PACKAGES[key].id}; the package layout changed.`);
        extracted.push(file);
      }
    }

    await mkdir(target.destDir, { recursive: true });
    const files = {};
    for (const file of extracted) {
      const name = basename(file);
      await copyFile(file, join(target.destDir, name));
      files[name] = await sha256File(join(target.destDir, name));
      log(`  ✓ ${name}`);
    }
    await mkdir(dirname(stampPath(target)), { recursive: true });
    await writeFile(stampPath(target), JSON.stringify({ manifest: manifestFingerprint(target), files }, null, 2));
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
}

/** Read env.yaml (seeding from the template if missing) as lines + the [start, end) of a section. */
async function loadEnvSection(envKey) {
  const envPath = join(REPO_ROOT, 'env.yaml');
  const templatePath = join(REPO_ROOT, 'env.yaml.template');
  let content;
  if (existsSync(envPath)) {
    content = await readFile(envPath, 'utf8');
  } else if (existsSync(templatePath)) {
    log(`  env.yaml missing - seeding from env.yaml.template`);
    content = await readFile(templatePath, 'utf8');
  } else {
    throw new Error(`Neither env.yaml nor env.yaml.template found in ${REPO_ROOT}`);
  }
  // Line-based edit (no YAML parse - preserves comments). Line-by-line because env.yaml may
  // mix CRLF/LF: `\r` counts as whitespace, so blank CRLF lines stay inside the section.
  const lines = content.split('\n');
  const headerIdx = lines.findIndex((l) => new RegExp(`^${envKey}:`).test(l));
  let endIdx = headerIdx + 1;
  if (headerIdx !== -1) while (endIdx < lines.length && !/^\S/.test(lines[endIdx])) endIdx++;
  const ortIdx = headerIdx === -1
    ? -1
    : lines.findIndex((l, i) => i > headerIdx && i < endIdx && /^\s+ORT_DYLIB_PATH\s*:/.test(l));
  return { envPath, lines, headerIdx, ortIdx };
}

/** Linux/WSL/macOS: write (or insert) ORT_DYLIB_PATH under the platform section. */
async function updateEnvYaml(envKey, absLibPath) {
  const { envPath, lines, headerIdx, ortIdx } = await loadEnvSection(envKey);
  const newLine = `  ORT_DYLIB_PATH: "${absLibPath.replaceAll('\\', '/')}"`;
  if (headerIdx === -1) {
    lines.push('', `${envKey}:`, newLine, '');
  } else if (ortIdx !== -1) {
    lines[ortIdx] = newLine;
  } else {
    lines.splice(headerIdx + 1, 0, newLine);
  }
  await writeFile(envPath, lines.join('\n'));
  log(`✓ env.yaml [${envKey}] ORT_DYLIB_PATH -> ${absLibPath}`);
}

/** Windows: drop an ORT_DYLIB_PATH that is the template placeholder or doesn't exist, so the
 *  staged bundled copy wins. A real existing path is kept (manual override). */
async function clearStaleWindowsOverride() {
  if (!existsSync(join(REPO_ROOT, 'env.yaml'))) return;
  const { envPath, lines, ortIdx } = await loadEnvSection('windows');
  if (ortIdx === -1) return;
  const raw = lines[ortIdx].replace(/^\s+ORT_DYLIB_PATH\s*:\s*/, '').trim().replace(/^["']|["']$/g, '');
  const expanded = raw.replace(/\$\{(\w+)\}/g, (_, v) => process.env[v] ?? '');
  const isPlaceholder = /\/YOU\//i.test(raw);
  if (!isPlaceholder && existsSync(expanded)) {
    log(`  env.yaml [windows] ORT_DYLIB_PATH override kept: ${raw}`);
    return;
  }
  lines.splice(ortIdx, 1);
  await writeFile(envPath, lines.join('\n'));
  log(`✓ Removed ${isPlaceholder ? 'placeholder' : 'non-existent'} env.yaml [windows] ORT_DYLIB_PATH (${raw}); using the bundled runtime`);
}

/**
 * macOS only: copy the dylib to `src-tauri/macos-frameworks/libonnxruntime.dylib`.
 * `tauri.conf.json5` declares it under `bundle.macOS.frameworks`, and `tauri_build::try_build`
 * validates that path EXISTS at compile time, so a fresh-clone `cargo build` fails with
 * "Library not found" unless it is staged first. Gitignored, never committed.
 */
async function stageMacosFramework(libPath) {
  const dest = join(REPO_ROOT, 'src-tauri', 'macos-frameworks', 'libonnxruntime.dylib');
  await mkdir(dirname(dest), { recursive: true });
  await copyFile(libPath, dest);
  log(`✓ Staged dylib for the macOS bundler: ${dest}`);
}

async function main() {
  const ci = process.argv.includes('--ci');
  const key = `${process.platform}-${process.arch}`;
  const target = TARGETS[key];
  if (!target) {
    throw new Error(
      `No prebuilt ONNX Runtime for ${key}. Build ORT ${ORT_VERSION} yourself and set ` +
      `ORT_DYLIB_PATH in env.yaml (see env.yaml.template).`
    );
  }

  if (await isUpToDate(target)) {
    log(`✓ ORT ${ORT_VERSION} already staged for ${key} (${target.destDir})`);
  } else {
    log(`Staging ONNX Runtime ${ORT_VERSION} for ${key}...`);
    await stage(target);
    log(`✓ Staged into ${target.destDir}`);
  }

  const libPath = target.lib ? join(target.destDir, target.lib) : null;
  if (key === 'darwin-arm64') await stageMacosFramework(libPath);
  if (ci) return;
  if (target.writesEnvPath) await updateEnvYaml(detectEnvKey(), libPath);
  else await clearStaleWindowsOverride();
}

main().catch((e) => {
  console.error(`[ensure-ort] FAILED: ${e instanceof Error ? e.message : e}`);
  process.exit(1);
});
