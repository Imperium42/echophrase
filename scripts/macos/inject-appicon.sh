#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

#
# Inject the compiled light/dark app-icon asset catalog into a built Echophrase
# .app and re-sign it.
#
# Tauri bundles only a single classic .icns (one fixed appearance). To get a Dock
# icon that auto-switches with the system theme we compile an Assets.car with
# light + dark variants (scripts/macos/build-appicon.mjs) and splice it in here:
#   1. copy Assets.car into Contents/Resources/
#   2. set CFBundleIconName=AppIcon in Contents/Info.plist (keep CFBundleIconFile
#      so pre-Tahoe still uses the .icns fallback)
#   3. re-sign - editing Resources/Info.plist invalidates Tauri's signature
#
# Usage: scripts/macos/inject-appicon.sh <path-to-Echophrase.app> [signing-identity]
#   signing-identity defaults to "-" (ad-hoc, matches tauri.conf.json5 dev signing).
#   In CI pass the Developer ID Application identity.
set -euo pipefail

APP="${1:?usage: inject-appicon.sh <Echophrase.app> [signing-identity]}"
IDENTITY="${2:--}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SCRIPT_DIR/../.." && pwd)"
CAR="$REPO/src-tauri/icons/.appicon-build/Assets.car"

if [[ ! -d "$APP" ]]; then echo "error: app bundle not found: $APP" >&2; exit 1; fi
if [[ ! -f "$CAR" ]]; then
  echo "inject-appicon: no Assets.car (full Xcode/actool absent) - skipping." >&2
  echo "  The bundled dark icon.icns is used as-is (no light/dark auto-switch)." >&2
  exit 0
fi

RES="$APP/Contents/Resources"
PLIST="$APP/Contents/Info.plist"

echo "inject-appicon: copying Assets.car -> $RES/"
cp -f "$CAR" "$RES/Assets.car"

echo "inject-appicon: setting CFBundleIconName=AppIcon in Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleIconName AppIcon" "$PLIST" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Add :CFBundleIconName string AppIcon" "$PLIST"

echo "inject-appicon: re-signing with identity '$IDENTITY'"
codesign --force --sign "$IDENTITY" --deep "$APP"
codesign --verify --deep --strict "$APP" && echo "inject-appicon: signature OK"

echo "inject-appicon: done. Re-launch the app (icon is cached - see notes if stale)."
