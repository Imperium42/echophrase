#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Windows test runner - runs the DirectML transcription test via PowerShell.
# Requires WIN_ECHOPHRASE_PATH to be set (native Windows path).
#
# Example:
#   export WIN_ECHOPHRASE_PATH='C:\dev\echophrase'
#   bash scripts/test-windows.sh

set -e

# ── Mode: DirectML transcription test ────────────────────────────────────────

run_transcription() {
    echo "=== Echophrase Windows DirectML Test ==="
    echo ""

    WIN_PROJECT_PATH="${WIN_ECHOPHRASE_PATH:-}"

    if [ -z "$WIN_PROJECT_PATH" ]; then
        echo "ERROR: WIN_ECHOPHRASE_PATH not set"
        echo ""
        echo "This test must run from a native Windows path (not WSL filesystem)."
        echo "Cargo has issues with lock files on network paths like \\\\wsl.localhost\\..."
        echo ""
        echo "To fix:"
        echo "  1. Clone the repo to a Windows path:"
        echo "     git clone https://github.com/Imperium42/echophrase.git C:\\dev\\echophrase"
        echo ""
        echo "  2. Set the environment variable:"
        echo "     export WIN_ECHOPHRASE_PATH='C:\\dev\\echophrase'"
        echo ""
        echo "  3. Run this script again"
        exit 1
    fi

    WIN_PATH=$(echo "$WIN_PROJECT_PATH" | sed 's|/|\\|g')
    WIN_SCRIPT="$WIN_PATH\\scripts\\win\\test-transcription.ps1"

    echo "Running test from: $WIN_PATH"
    echo "Script: $WIN_SCRIPT"
    echo ""

    pwsh.exe -ExecutionPolicy Bypass -Command "
        Set-Location '$WIN_PATH'
        & '$WIN_SCRIPT'
    "

    echo ""
    echo "=== Test Complete ==="
}

run_transcription
