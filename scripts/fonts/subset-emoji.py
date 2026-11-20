#!/usr/bin/env python3

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0
"""Build the bundled emoji web font (static/fonts/emoji/) + its @font-face CSS.

Purpose: the app window used to load Noto Color Emoji from fonts.googleapis.com,
which made every launch hit Google (and leak the user's IP) just to render a
couple of glyphs. This script replaces that with a local, subsetted copy:

  1. Scans src/ (.svelte, .ts, .js, .json, .html) for every emoji codepoint the
     app can render.
  2. Fetches the Google Fonts CSS for Noto Color Emoji with a Chromium UA so we
     get the same COLRv1 woff2 shards WebView2/WebKit were already loading.
  3. Subsets each shard to the codepoints it covers that we actually use
     (pyftsubset keeps COLR/CPAL), drops shards that contribute nothing.
  4. Writes static/fonts/emoji/noto-color-emoji-<n>.woff2 and
     src/lib/styles/emoji-font.css with matching @font-face + unicode-range.

Re-run whenever a new emoji is added to UI copy:
    pip install fonttools brotli
    python3 scripts/fonts/subset-emoji.py

Font: Noto Color Emoji, (c) Google, SIL Open Font License 1.1
(static/fonts/emoji/OFL.txt). No network access is needed by the app itself.
"""
from __future__ import annotations

import pathlib
import re
import subprocess
import sys
import tempfile
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[2]
SRC = ROOT / "src"
OUT_DIR = ROOT / "static" / "fonts" / "emoji"
CSS_OUT = ROOT / "src" / "lib" / "styles" / "emoji-font.css"
FAMILY = "Noto Color Emoji"
CSS_URL = "https://fonts.googleapis.com/css2?family=Noto+Color+Emoji&display=swap"
# Chromium UA -> Google serves the COLRv1 woff2 shards (what WebView2 renders).
UA = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/130.0 Safari/537.36"
)
SCAN_SUFFIXES = {".svelte", ".ts", ".js", ".json", ".html"}
# Emoji-ish blocks. Plain text symbols (arrows, ⌘, ©) fall out naturally because
# the emoji font has no glyph for them and pyftsubset just skips them.
EMOJI_RE = re.compile(
    "[\U0001F000-\U0001FAFF☀-➿⬀-⯿⌀-⏿"
    "〰〽㊗㊙©®™ℹⓂ]"
)
# Always keep: VS16 (emoji presentation) + ZWJ so sequences keep working.
ALWAYS = {0xFE0F, 0x200D}


def scan_codepoints() -> set[int]:
    found: set[int] = set()
    for p in SRC.rglob("*"):
        if p.is_file() and p.suffix in SCAN_SUFFIXES and ".test." not in p.name:
            for m in EMOJI_RE.finditer(p.read_text(encoding="utf-8", errors="ignore")):
                found.add(ord(m.group()))
    return found | ALWAYS


def fetch(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def parse_ranges(spec: str) -> set[int]:
    cps: set[int] = set()
    for part in spec.split(","):
        part = part.strip().upper().removeprefix("U+")
        if "-" in part:
            a, b = part.split("-")
            cps.update(range(int(a, 16), int(b, 16) + 1))
        else:
            cps.add(int(part, 16))
    return cps


def main() -> int:
    wanted = scan_codepoints()
    print(f"{len(wanted)} candidate codepoints in src/")

    css = fetch(CSS_URL).decode("utf-8")
    shards = re.findall(r"src: url\((\S+?)\) format\('woff2'\);\s*unicode-range: ([^;]+);", css)
    if not shards:
        print("could not parse Google Fonts CSS", file=sys.stderr)
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for old in OUT_DIR.glob("noto-color-emoji-*.woff2"):
        old.unlink()

    faces: list[str] = []
    with tempfile.TemporaryDirectory() as tmp:
        for i, (url, ranges) in enumerate(shards):
            keep = sorted(parse_ranges(ranges) & wanted)
            real = [c for c in keep if c not in ALWAYS]
            if not real:
                continue
            src_path = pathlib.Path(tmp) / f"shard{i}.woff2"
            src_path.write_bytes(fetch(url))
            out_path = OUT_DIR / f"noto-color-emoji-{i}.woff2"
            subprocess.run(
                [
                    sys.executable, "-m", "fontTools.subset", str(src_path),
                    f"--unicodes={','.join(f'U+{c:04X}' for c in keep)}",
                    "--flavor=woff2", "--layout-features=*", "--no-hinting",
                    f"--output-file={out_path}",
                ],
                check=True,
            )
            size = out_path.stat().st_size
            print(f"shard {i}: {len(real)} glyphs -> {out_path.name} ({size} B)")
            faces.append(
                "@font-face {\n"
                f"  font-family: '{FAMILY}';\n"
                "  font-style: normal;\n"
                "  font-weight: 400;\n"
                "  font-display: swap;\n"
                f"  src: url('/fonts/emoji/{out_path.name}') format('woff2');\n"
                f"  unicode-range: {', '.join(f'U+{c:04X}' for c in keep)};\n"
                "}\n"
            )

    header = (
        "/* GENERATED by scripts/fonts/subset-emoji.py - do not edit by hand.\n"
        "   Local, subsetted Noto Color Emoji (OFL 1.1, static/fonts/emoji/OFL.txt)\n"
        "   so the app window never requests fonts from Google. Re-run the script\n"
        "   after adding new emoji to UI copy. */\n\n"
    )
    CSS_OUT.parent.mkdir(parents=True, exist_ok=True)
    CSS_OUT.write_text(header + "\n".join(faces), encoding="utf-8")
    print(f"wrote {CSS_OUT.relative_to(ROOT)} ({len(faces)} faces)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
