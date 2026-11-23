#!/usr/bin/env node

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Generate the third-party OSS attribution data that the marketing site renders
 * at /attribution, plus the root ATTRIBUTION.md.
 *
 * Sources (both from THIS repo, which owns the lockfiles):
 *   1. Rust crates  - `cargo about generate src-tauri/about.hbs` walks the full
 *      shipped dependency graph from Cargo.lock and emits one delimited block per
 *      unique license text (see about.hbs). about.toml's `accepted` list is the
 *      copyleft guard - cargo-about fails if a GPL/AGPL/LGPL crate appears.
 *   2. npm packages - `vite build` runs rollup-license-plugin (see vite.config.js)
 *      which writes oss-licenses.json listing ONLY the packages actually bundled
 *      into the shipped webview, with their license text.
 *
 * Output (committed, so the www host needs no Rust/npm toolchain):
 *   - ../echophrase-www/src/lib/data/licenses.generated.json  (rendered by the page)
 *   - ./ATTRIBUTION.md                                        (repo-root artifact)
 *
 * Usage:
 *   node scripts/gen-attribution.mjs [--skip-npm-build]
 *   (--skip-npm-build reuses an existing static/oss-licenses.json for fast dev
 *    iteration; the release path always rebuilds for freshness.)
 */

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const clientRoot = resolve(scriptDir, "..");
const srcTauri = resolve(clientRoot, "src-tauri");
const wwwDataFile = resolve(
  clientRoot,
  "../echophrase-www/src/lib/data/licenses.generated.json",
);
const attributionMd = resolve(clientRoot, "ATTRIBUTION.md");
// Full license texts, bundled into the installers via tauri.conf.json5 `resources`.
const noticesTxt = resolve(srcTauri, "licenses/THIRD-PARTY-NOTICES.txt");
const npmManifest = resolve(clientRoot, "static/oss-licenses.json");

const skipNpmBuild = process.argv.includes("--skip-npm-build");

// Copyleft licenses that must never ship. The Rust side is already guarded by
// about.toml's `accepted` allowlist; this is the second gate for npm packages.
const COPYLEFT = /\b(?:A?GPL|LGPL)(?:[-\s]|$)/i;

/** SPDX id -> friendly display name for section headings. */
const SPDX_NAMES = {
  "0BSD": "BSD Zero Clause License",
  "Apache-2.0": "Apache License 2.0",
  "Apache-2.0 WITH LLVM-exception": "Apache License 2.0 with LLVM Exception",
  "BSD-2-Clause": "BSD 2-Clause License",
  "BSD-3-Clause": "BSD 3-Clause License",
  "BSL-1.0": "Boost Software License 1.0",
  "CC0-1.0": "Creative Commons Zero v1.0 Universal",
  "CDLA-Permissive-2.0": "Community Data License Agreement - Permissive 2.0",
  ISC: "ISC License",
  MIT: "MIT License",
  "MIT-0": "MIT No Attribution License",
  "MPL-2.0": "Mozilla Public License 2.0",
  OpenSSL: "OpenSSL License",
  "Unicode-3.0": "Unicode License v3",
  "Unicode-DFS-2016": "Unicode License Agreement - DFS 2016",
  Zlib: "zlib License",
};

/** Normalize a license text for dedup (collapse whitespace). */
function textKey(text) {
  return text.trim().replace(/\s+/g, " ");
}

/**
 * Parse the delimited output of `cargo about generate about.hbs` into blocks.
 * @returns {Array<{spdx:string,name:string,text:string,packages:Array<{name:string,version:string,ecosystem:string}>}>}
 */
function parseCargoAbout(raw) {
  const blocks = [];
  for (const chunk of raw.split("@@BLOCK@@").slice(1)) {
    const idMatch = chunk.match(/@@ID@@(.*)/);
    const nameMatch = chunk.match(/@@NAME@@(.*)/);
    const pkgsSection = chunk.slice(
      chunk.indexOf("@@PKGS@@") + "@@PKGS@@".length,
      chunk.indexOf("@@TEXT@@"),
    );
    const text = chunk.slice(
      chunk.indexOf("@@TEXT@@") + "@@TEXT@@".length,
      chunk.indexOf("@@ENDTEXT@@"),
    );
    const spdx = (idMatch?.[1] ?? "").trim();
    // Skip the first-party workspace crate (no license, empty id).
    if (!spdx) continue;
    const packages = pkgsSection
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .map((line) => {
        const [name, version] = line.split("\t");
        return { name, version: version ?? "", ecosystem: "cargo" };
      });
    if (packages.length === 0) continue;
    blocks.push({
      spdx,
      name: (nameMatch?.[1] ?? spdx).trim(),
      text: text.replace(/^\n/, "").replace(/\n$/, ""),
      packages,
    });
  }
  return blocks;
}

/**
 * Convert the rollup-license-plugin manifest into the same block shape (one
 * block per npm package; identical texts get merged during dedup).
 */
function parseNpmManifest(entries) {
  return entries.map((e) => ({
    spdx: e.license,
    name: SPDX_NAMES[e.license] ?? e.license,
    text: (e.licenseText ?? "").trim(),
    packages: [{ name: e.name, version: e.version, ecosystem: "npm" }],
  }));
}

/** Merge blocks that share the same SPDX id AND identical license text. */
function dedupeBlocks(blocks) {
  const byKey = new Map();
  for (const b of blocks) {
    const key = `${b.spdx}::${textKey(b.text)}`;
    const existing = byKey.get(key);
    if (existing) {
      const seen = new Set(
        existing.packages.map((p) => `${p.name}@${p.version}`),
      );
      for (const p of b.packages) {
        if (!seen.has(`${p.name}@${p.version}`)) existing.packages.push(p);
      }
    } else {
      byKey.set(key, { ...b, packages: [...b.packages] });
    }
  }
  const merged = [...byKey.values()];
  for (const b of merged) {
    b.packages.sort((a, c) => a.name.localeCompare(c.name));
  }
  // Sort blocks by SPDX id, then by first package name for stable diffs.
  merged.sort(
    (a, c) =>
      a.spdx.localeCompare(c.spdx) ||
      a.packages[0].name.localeCompare(c.packages[0].name),
  );
  return merged;
}

/** Render the repo-root ATTRIBUTION.md library section from merged blocks. */
function renderMarkdown(blocks, counts) {
  const lines = [];
  lines.push("# Third-Party Attribution (Libraries)");
  lines.push("");
  lines.push(
    "> Auto-generated by `scripts/gen-attribution.mjs` - do not edit by hand.",
  );
  lines.push(
    `> ${counts.cargo} Rust crates + ${counts.npm} bundled npm packages across ${blocks.length} distinct license notices.`,
  );
  lines.push("");
  lines.push(
    "The curated machine-learning model attributions live on the website at",
  );
  lines.push("https://echophrase.com/attribution (runtime model downloads are");
  lines.push("not captured by dependency tooling).");
  lines.push("");
  // Native runtimes shipped as prebuilt binaries (not crates or npm packages,
  // so cargo-about / rollup-license-plugin can't see them). Keep in sync with
  // what scripts/ensure-ort.mjs stages into the installer.
  lines.push("## Bundled native runtimes");
  lines.push("");
  lines.push(
    "- **ONNX Runtime** (`onnxruntime.dll` / `libonnxruntime.dylib`) - Microsoft, MIT License. https://github.com/microsoft/onnxruntime",
  );
  lines.push(
    "- **DirectML** (`DirectML.dll`, Windows) - Microsoft, redistributed under the Microsoft DirectML license (proprietary, redistributable; not open source). https://www.nuget.org/packages/Microsoft.AI.DirectML",
  );
  lines.push(
    "- **Silero VAD** (`silero_vad.onnx`, embedded) - Silero Team, MIT License. https://github.com/snakers4/silero-vad",
  );
  lines.push(
    "- **Whisper mel filterbanks** (`mel_filters*.bytes`, embedded) - OpenAI, MIT License. https://github.com/openai/whisper",
  );
  lines.push(
    "- **Noto Color Emoji subset** (`static/fonts/emoji`) - Google, SIL Open Font License 1.1 (OFL-1.1). https://github.com/googlefonts/noto-emoji",
  );
  lines.push("");
  const bySpdx = new Map();
  for (const b of blocks) {
    if (!bySpdx.has(b.spdx)) bySpdx.set(b.spdx, []);
    bySpdx.get(b.spdx).push(b);
  }
  for (const [spdx, group] of [...bySpdx.entries()].sort()) {
    const pkgCount = group.reduce((n, b) => n + b.packages.length, 0);
    lines.push(`## ${SPDX_NAMES[spdx] ?? spdx} (${spdx}) - ${pkgCount}`);
    lines.push("");
    const names = group
      .flatMap((b) => b.packages.map((p) => p.name))
      .sort()
      .join(", ");
    lines.push(names);
    lines.push("");
  }
  return lines.join("\n") + "\n";
}

/** Render every license text in full (MIT/BSD/Apache require the notice to ship). */
function renderNotices(blocks) {
  const out = [
    "Echophrase third-party notices",
    "Auto-generated by scripts/gen-attribution.mjs - do not edit by hand.",
    "",
    "Bundled native runtimes ship their own notices next to this file:",
    "  onnxruntime-LICENSE.txt, onnxruntime-ThirdPartyNotices.txt (ONNX Runtime, MIT)",
    "  DirectML-LICENSE.txt, DirectML-ThirdPartyNotices.txt (DirectML, Windows only)",
    "Silero VAD (embedded silero_vad.onnx): MIT License, Copyright (c) 2020-present Silero Team.",
    "Whisper mel filterbanks (embedded): OpenAI, MIT License. https://github.com/openai/whisper",
    "Noto Color Emoji (subset): Google, SIL Open Font License 1.1 (see LICENSES/OFL-1.1.txt in the source repo).",
    "",
  ];
  for (const b of blocks) {
    out.push("=".repeat(78));
    out.push(`${b.name} (${b.spdx})`);
    out.push(`Used by: ${b.packages.map((p) => `${p.name} ${p.version}`.trim()).join(", ")}`);
    out.push("-".repeat(78));
    out.push(b.text.trim());
    out.push("");
  }
  return out.join("\n") + "\n";
}

function main() {
  // 1. Rust crates via cargo-about.
  console.log("[attribution] running cargo about generate...");
  // Write to a file (-o): on Windows cargo-about refuses to emit to a pipe
  // ("should not redirect its output in powershell").
  const cargoOut = join(mkdtempSync(join(tmpdir(), "echo-attr-")), "cargo-about.txt");
  execFileSync("cargo", ["about", "generate", "about.hbs", "-o", cargoOut], {
    cwd: srcTauri,
    stdio: ["ignore", "inherit", "inherit"],
  });
  const cargoRaw = readFileSync(cargoOut, "utf8");
  rmSync(dirname(cargoOut), { recursive: true, force: true });
  const cargoBlocks = parseCargoAbout(cargoRaw);
  console.log(`[attribution] cargo-about: ${cargoBlocks.length} license blocks`);

  // 2. npm bundle via vite build + rollup-license-plugin.
  if (!skipNpmBuild || !existsSync(npmManifest)) {
    console.log("[attribution] running vite build for npm manifest...");
    execFileSync("bunx", ["--bun", "vite", "build"], {
      cwd: clientRoot,
      encoding: "utf8",
      stdio: "inherit",
      // Enables the license plugin in vite.config.js (gated so it never runs
      // during a normal dev/release build - see the plugin comment there).
      env: { ...process.env, GEN_ATTRIBUTION: "1" },
    });
  }
  if (!existsSync(npmManifest)) {
    throw new Error(`npm manifest not found at ${npmManifest}`);
  }
  const npmEntries = JSON.parse(readFileSync(npmManifest, "utf8"));
  const npmBlocks = parseNpmManifest(npmEntries);
  console.log(`[attribution] npm bundle: ${npmEntries.length} packages`);

  // 3. Copyleft guard (Rust already guarded by about.toml; check npm + belt-and-suspenders).
  const allBlocks = [...cargoBlocks, ...npmBlocks];
  const offenders = allBlocks.filter((b) => COPYLEFT.test(b.spdx));
  if (offenders.length > 0) {
    const list = offenders
      .map((b) => `${b.spdx}: ${b.packages.map((p) => p.name).join(", ")}`)
      .join("\n  ");
    throw new Error(
      `Copyleft license(s) detected - refusing to generate:\n  ${list}`,
    );
  }

  // 4. Merge + dedupe.
  const blocks = dedupeBlocks(allBlocks);
  const counts = {
    cargo: cargoBlocks.reduce((n, b) => n + b.packages.length, 0),
    npm: npmEntries.length,
    licenseBlocks: blocks.length,
  };
  const spdxIds = [...new Set(blocks.map((b) => b.spdx))].sort();

  const out = {
    // NOTE: no timestamp - keeps the committed artifact deterministic so a
    // no-change regeneration produces no git diff.
    counts,
    spdxIds,
    blocks,
  };

  writeFileSync(attributionMd, renderMarkdown(blocks, counts));
  writeFileSync(noticesTxt, renderNotices(blocks));
  console.log(`[attribution] wrote ${noticesTxt}`);
  console.log(`[attribution] wrote ${attributionMd}`);
  // The website repo is a maintainer-only sibling checkout; public clones
  // don't have it, so only ATTRIBUTION.md is required.
  if (existsSync(dirname(wwwDataFile))) {
    writeFileSync(wwwDataFile, JSON.stringify(out, null, 2) + "\n");
    console.log(`[attribution] wrote ${wwwDataFile}`);
  } else {
    console.log(`[attribution] skipped ${wwwDataFile} (website repo not checked out)`);
  }
  console.log(
    `[attribution] done: ${counts.cargo} crates + ${counts.npm} npm across ${blocks.length} notices (${spdxIds.length} SPDX ids: ${spdxIds.join(", ")})`,
  );
}

main();
