#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Dev launcher dispatcher - detects the current platform and execs the right worker script.
#
# Platforms handled:
#   WSL   -> scripts/wsl/dev.sh  (CPU-only)
#   macOS -> scripts/mac/dev.sh  (Metal / CoreML)
#   Linux -> bun run tauri:dev   (native Linux, no separate worker)
#   Other -> bun run tauri:dev   (best-effort fallback)
#
# Worker scripts keep platform-specific logic (env var setup, pre-flight checks, etc.)
# so this dispatcher stays simple: detect once, hand off, done.

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# ── Platform detection ────────────────────────────────────────────────────────

detect_platform() {
    local os
    os="$(uname -s)"

    case "$os" in
        Darwin)
            echo "macos"
            return
            ;;
        Linux)
            # Check for WSL via /proc/version and env vars
            if [[ -f /proc/version ]] && grep -qi "microsoft\|wsl" /proc/version 2>/dev/null; then
                echo "wsl"
                return
            fi
            if [[ -n "${WSL_DISTRO_NAME:-}" ]] || [[ -n "${WSL_INTEROP:-}" ]]; then
                echo "wsl"
                return
            fi
            echo "linux"
            return
            ;;
        *)
            echo "unknown"
            return
            ;;
    esac
}

PLATFORM="$(detect_platform)"
echo "Detected platform: $PLATFORM"

# ── Dispatch ──────────────────────────────────────────────────────────────────

case "$PLATFORM" in
    wsl)
        echo "Dispatching to scripts/wsl/dev.sh..."
        exec bash "$SCRIPT_DIR/wsl/dev.sh" "$@"
        ;;
    macos)
        echo "Dispatching to scripts/mac/dev.sh..."
        exec bash "$SCRIPT_DIR/mac/dev.sh" "$@"
        ;;
    linux)
        echo "Native Linux - launching via bun run tauri:dev..."
        exec bun run tauri:dev "$@"
        ;;
    *)
        echo "Warning: unknown platform ($PLATFORM) - attempting bun run tauri:dev..."
        exec bun run tauri:dev "$@"
        ;;
esac
