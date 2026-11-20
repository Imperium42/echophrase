#!/usr/bin/env bun

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Build the macOS app (Dock/Finder) icon with light/dark auto-switching.
 *
 * macOS does NOT auto-add the rounded-square ("squircle") tile - each app bakes
 * its own background + shape. And Tauri 2.x only bundles a single classic .icns,
 * which is one fixed appearance. To get a tile that auto-switches with the system
 * theme we compile an asset catalog (Assets.car) with light + dark appearance
 * variants via `actool`, then inject it into the built .app post-build
 * (see scripts/macos/inject-appicon.sh).
 *
 * This script:
 *   1. Composes light + dark icon SVGs (squircle gradient tile + brand glyph).
 *   2. Rasterizes them to all macOS icon sizes with sharp.
 *   3. Writes an Assets.xcassets/AppIcon.appiconset with default + dark entries.
 *   4. Runs actool -> Assets.car (+ AppIcon.icns) into .appicon-build/.
 *   5. Regenerates the classic src-tauri/icons/icon.icns from the LIGHT artwork
 *      (single-appearance fallback for pre-Tahoe and the dev build).
 *
 * macOS only (uses actool/iconutil/sips). Run: `bun run icons:mac`.
 *
 * TODO(contrast): the brand gradient glyph is currently identical on the light
 * and dark tiles. If it reads weak on either background, revisit per-mode glyph
 * contrast here (e.g. brighten/whiten the glyph on dark, deepen it on light)
 * instead of sharing one GLYPH_BARS gradient.
 */

import { mkdir, rm, writeFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const __dirname = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(__dirname, '..', '..'); // echophrase/
const ICONS_DIR = join(REPO, 'src-tauri', 'icons');
// Committed macOS icon source-of-truth + previews (both appearances). Grouped in
// a mac/ dir so the platform is obvious.
const MAC_DIR = join(ICONS_DIR, 'mac');
const XCASSETS = join(ICONS_DIR, 'Assets.xcassets');
const APPICONSET = join(XCASSETS, 'AppIcon.appiconset');
const BUILD_DIR = join(ICONS_DIR, '.appicon-build'); // gitignored

// --- Geometry: Apple macOS icon grid -------------------------------------
// Icons are not full-bleed: a 1024 canvas holds an 824 tile centered with
// 100px margins (room for the system drop shadow). Corner radius ~0.2237*side.
const CANVAS = 1024;
const TILE = 824;
const MARGIN = (CANVAS - TILE) / 2; // 100
const RADIUS = Math.round(TILE * 0.2237); // ~184
const GLYPH_FRACTION = 0.64; // glyph WIDTH as a fraction of the tile width

// --- Brand glyph (from static/images/echophrase-icon.svg, 48x48 viewBox) --
// 5 soundwave bars, cyan -> violet -> pink, bottom-to-top gradient.
const GLYPH_BARS = [
  { x: 6, y: 19, w: 4, h: 10 },
  { x: 14, y: 15, w: 4, h: 18 },
  { x: 22, y: 10, w: 4, h: 28 },
  { x: 30, y: 13, w: 4, h: 22 },
  { x: 38, y: 17, w: 4, h: 14 },
];
const GLYPH_VIEWBOX = 48;

// Tight bounding box of the bars within the 48x48 viewBox (the source glyph does
// NOT fill its viewBox - bars span ~36x28). We crop to this so GLYPH_FRACTION is
// the real on-tile size, not "fraction of a mostly-empty box".
const GLYPH_BBOX = (() => {
  const minX = Math.min(...GLYPH_BARS.map((b) => b.x));
  const minY = Math.min(...GLYPH_BARS.map((b) => b.y));
  const maxX = Math.max(...GLYPH_BARS.map((b) => b.x + b.w));
  const maxY = Math.max(...GLYPH_BARS.map((b) => b.y + b.h));
  return { minX, minY, w: maxX - minX, h: maxY - minY };
})();

/** Compose the full icon SVG for one appearance. */
function iconSvg({ bgStops }) {
  // Scale the glyph's *bounding box* (not its viewBox) to GLYPH_FRACTION of the
  // tile's wider dimension, then center it on the canvas. Far less dead space.
  const scale = (TILE * GLYPH_FRACTION) / GLYPH_BBOX.w;
  const drawnW = GLYPH_BBOX.w * scale;
  const drawnH = GLYPH_BBOX.h * scale;
  const tx = (CANVAS - drawnW) / 2 - GLYPH_BBOX.minX * scale;
  const ty = (CANVAS - drawnH) / 2 - GLYPH_BBOX.minY * scale;
  const bars = GLYPH_BARS.map(
    (b) =>
      `<rect x="${b.x}" y="${b.y}" width="${b.w}" height="${b.h}" rx="2" fill="url(#glyph)" />`,
  ).join('\n      ');
  const stops = bgStops
    .map((s) => `<stop offset="${s.o}" stop-color="${s.c}" />`)
    .join('\n      ');
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${CANVAS}" height="${CANVAS}" viewBox="0 0 ${CANVAS} ${CANVAS}">
  <defs>
    <linearGradient id="tile" x1="0" y1="0" x2="0" y2="1">
      ${stops}
    </linearGradient>
    <linearGradient id="glyph" x1="0%" y1="100%" x2="0%" y2="0%">
      <stop offset="0%" stop-color="#06b6d4" />
      <stop offset="50%" stop-color="#8b5cf6" />
      <stop offset="100%" stop-color="#ec4899" />
    </linearGradient>
  </defs>
  <rect x="${MARGIN}" y="${MARGIN}" width="${TILE}" height="${TILE}" rx="${RADIUS}" ry="${RADIUS}" fill="url(#tile)" />
  <g transform="translate(${tx}, ${ty}) scale(${scale})">
      ${bars}
  </g>
</svg>`;
}

// Whiteish (light mode) and blackish (dark mode) gradient tiles.
const LIGHT_SVG = iconSvg({
  bgStops: [
    { o: '0%', c: '#ffffff' },
    { o: '100%', c: '#e7e9ee' },
  ],
});
const DARK_SVG = iconSvg({
  bgStops: [
    { o: '0%', c: '#2a2a2e' },
    { o: '100%', c: '#0b0b0d' },
  ],
});

// macOS app icon sizes (idiom "mac"): pt size + scale.
const ENTRIES = [
  { size: 16, scale: 1 },
  { size: 16, scale: 2 },
  { size: 32, scale: 1 },
  { size: 32, scale: 2 },
  { size: 128, scale: 1 },
  { size: 128, scale: 2 },
  { size: 256, scale: 1 },
  { size: 256, scale: 2 },
  { size: 512, scale: 1 },
  { size: 512, scale: 2 },
];

function px(e) {
  return e.size * e.scale;
}

async function renderPng(svg, pixels, outPath) {
  await sharp(Buffer.from(svg)).resize(pixels, pixels).png().toFile(outPath);
}

function run(cmd, args) {
  const r = spawnSync(cmd, args, { stdio: 'inherit' });
  if (r.status !== 0) {
    throw new Error(`${cmd} ${args.join(' ')} -> exit ${r.status}`);
  }
}

async function main() {
  if (process.platform !== 'darwin') {
    console.error('build-appicon: macOS only (needs actool/iconutil/sips).');
    process.exit(1);
  }

  // 0. Persist the composed SVGs + rendered previews (both appearances) under
  //    src-tauri/icons/mac/ as committed source-of-truth artifacts.
  await mkdir(MAC_DIR, { recursive: true });
  await writeFile(join(MAC_DIR, 'echophrase-appicon-light.svg'), LIGHT_SVG);
  await writeFile(join(MAC_DIR, 'echophrase-appicon-dark.svg'), DARK_SVG);
  await renderPng(LIGHT_SVG, 512, join(MAC_DIR, 'preview-light.png'));
  await renderPng(DARK_SVG, 512, join(MAC_DIR, 'preview-dark.png'));
  // Side-by-side preview on neutral gray (light | dark) - the one shared in review.
  {
    const W = 1120, H = 560, pad = 48, s = 464;
    const bg = Buffer.from(
      `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><rect width="${W}" height="${H}" fill="#7a7a7e"/></svg>`,
    );
    const l = await sharp(Buffer.from(LIGHT_SVG)).resize(s, s).png().toBuffer();
    const d = await sharp(Buffer.from(DARK_SVG)).resize(s, s).png().toBuffer();
    await sharp(bg)
      .composite([
        { input: l, left: pad, top: (H - s) / 2 },
        { input: d, left: W - pad - s, top: (H - s) / 2 },
      ])
      .png()
      .toFile(join(MAC_DIR, 'preview-sidebyside.png'));
  }

  // 1. Fresh appiconset.
  await rm(XCASSETS, { recursive: true, force: true });
  await mkdir(APPICONSET, { recursive: true });

  const images = [];
  const seen = new Set();
  for (const e of ENTRIES) {
    const p = px(e);
    for (const [appearance, svg, suffix] of [
      [null, LIGHT_SVG, 'light'],
      ['dark', DARK_SVG, 'dark'],
    ]) {
      const filename = `icon_${p}_${suffix}.png`;
      if (!seen.has(filename)) {
        await renderPng(svg, p, join(APPICONSET, filename));
        seen.add(filename);
      }
      const entry = {
        idiom: 'mac',
        size: `${e.size}x${e.size}`,
        scale: `${e.scale}x`,
        filename,
      };
      if (appearance === 'dark') {
        entry.appearances = [{ appearance: 'luminosity', value: 'dark' }];
      }
      images.push(entry);
    }
  }

  await writeFile(
    join(APPICONSET, 'Contents.json'),
    JSON.stringify({ images, info: { author: 'echophrase', version: 1 } }, null, 2),
  );

  await rm(BUILD_DIR, { recursive: true, force: true });
  await mkdir(BUILD_DIR, { recursive: true });

  // 2. Regenerate the classic single-appearance .icns fallback from DARK art.
  //    Done FIRST and unconditionally - this gives the squircle tile in the Dock
  //    even where the Assets.car (auto light/dark) step can't run. We use the DARK
  //    tile because it reads cleanly on both light and dark Docks (a whiteish tile
  //    looks glary in Dark mode); the Assets.car still provides true per-mode art.
  const FALLBACK_SVG = DARK_SVG;
  const iconset = join(BUILD_DIR, 'icon.iconset');
  await mkdir(iconset, { recursive: true });
  const icnsSizes = [
    ['icon_16x16.png', 16],
    ['icon_16x16@2x.png', 32],
    ['icon_32x32.png', 32],
    ['icon_32x32@2x.png', 64],
    ['icon_128x128.png', 128],
    ['icon_128x128@2x.png', 256],
    ['icon_256x256.png', 256],
    ['icon_256x256@2x.png', 512],
    ['icon_512x512.png', 512],
    ['icon_512x512@2x.png', 1024],
  ];
  for (const [name, p] of icnsSizes) {
    await renderPng(FALLBACK_SVG, p, join(iconset, name));
  }
  run('iconutil', ['-c', 'icns', iconset, '-o', join(ICONS_DIR, 'icon.icns')]);
  // NOTE: icon.icns is the macOS Dock asset. We deliberately leave the
  // cross-platform icon.png / 32x32.png etc. untouched (bare glyph) so this only
  // changes the macOS Dock icon, not Linux/Windows window icons or the tray.
  console.log('build-appicon: wrote squircle icon.icns (dark fallback)');

  // 3. Compile the catalog -> Assets.car (+ AppIcon.icns) for auto light/dark.
  //    Requires FULL Xcode (actool). Command Line Tools alone is not enough, so
  //    this is best-effort: where Xcode is absent we still ship the .icns tile.
  if (!hasXcodeActool()) {
    console.warn(
      '\nbuild-appicon: WARNING - full Xcode (actool) not found.\n' +
        '  Skipped Assets.car -> the Dock icon will show the squircle tile but will\n' +
        '  NOT auto-switch light/dark. Install Xcode (then `sudo xcode-select -s\n' +
        '  /Applications/Xcode.app`) or run this in CI, and re-run `bun run icons:mac`.',
    );
    return;
  }
  run('actool', [
    XCASSETS,
    '--compile',
    BUILD_DIR,
    '--platform',
    'macosx',
    '--minimum-deployment-target',
    '13.3',
    '--app-icon',
    'AppIcon',
    '--output-partial-info-plist',
    join(BUILD_DIR, 'partial.plist'),
    '--errors',
    '--warnings',
    '--notices',
  ]);
  if (!existsSync(join(BUILD_DIR, 'Assets.car'))) {
    throw new Error('actool ran but did not produce Assets.car');
  }
  console.log('\nbuild-appicon: done (with auto light/dark Assets.car).');
  console.log(`  Assets.car -> ${join(BUILD_DIR, 'Assets.car')}`);
  console.log('  Next: build the app, then run scripts/macos/inject-appicon.sh');
}

/** True only when the active developer dir is a full Xcode (actool usable). */
function hasXcodeActool() {
  const probe = spawnSync('actool', ['--version'], { stdio: 'ignore' });
  return probe.status === 0;
}

await main();
