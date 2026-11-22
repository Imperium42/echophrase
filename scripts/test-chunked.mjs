// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

// Chunked frontend test runner. Run from repo root: `bun run test:chunked`.
//
// Splits the vitest files into small chunks and runs each with a hard
// timeout. A hung chunk is killed (whole process tree) and named, so a
// deadlock costs minutes, not a night, and never leaves orphaned workers.
// Uses the Node runtime (see vitest.config.js for why not --bun).
//
// Env: TEST_CHUNK_SIZE (default 5 files), TEST_CHUNK_TIMEOUT_S (default 120).

import { spawn, spawnSync } from 'node:child_process';
import { readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const CHUNK_SIZE = Number(process.env.TEST_CHUNK_SIZE ?? 5);
const TIMEOUT_MS = Number(process.env.TEST_CHUNK_TIMEOUT_S ?? 120) * 1000;
const ROOTS = ['src', 'tests'];
const TEST_RE = /\.(test|spec)\.(js|ts)$/;

function walk(dir, out) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const name of entries) {
    if (name === 'node_modules') continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (TEST_RE.test(name)) out.push(p.replaceAll('\\', '/'));
  }
  return out;
}

function killTree(pid) {
  if (process.platform === 'win32') spawnSync('taskkill', ['/PID', String(pid), '/T', '/F']);
  else {
    try {
      process.kill(-pid, 'SIGKILL');
    } catch {
      /* already gone */
    }
  }
}

function runChunk(files) {
  return new Promise(resolve => {
    const child = spawn('bunx', ['vitest', 'run', ...files], {
      stdio: 'inherit',
      shell: process.platform === 'win32',
      detached: process.platform !== 'win32',
    });
    const timer = setTimeout(() => {
      killTree(child.pid);
      resolve('timeout');
    }, TIMEOUT_MS);
    child.on('exit', code => {
      clearTimeout(timer);
      resolve(code === 0 ? 'pass' : 'fail');
    });
  });
}

const files = ROOTS.flatMap(r => walk(r, [])).sort();
const chunks = [];
for (let i = 0; i < files.length; i += CHUNK_SIZE) chunks.push(files.slice(i, i + CHUNK_SIZE));

const results = [];
for (const [i, chunk] of chunks.entries()) {
  console.log(`\n=== chunk ${i + 1}/${chunks.length}: ${chunk.join(' ')}`);
  results.push({ chunk, status: await runChunk(chunk) });
}

console.log('\n=== summary');
for (const { chunk, status } of results) {
  if (status !== 'pass') console.log(`${status.toUpperCase()}: ${chunk.join(' ')}`);
}
const bad = results.filter(r => r.status !== 'pass').length;
console.log(`${results.length - bad}/${results.length} chunks passed`);
process.exit(bad ? 1 : 0);
