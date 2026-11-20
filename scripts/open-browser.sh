#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Open Tauri dev server in browser
# Works across WSL, Linux, and macOS

set -e

# Default URL
URL="${1:-http://localhost:${DEV_PORT:-8420}}"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🌐 Opening browser: ${URL}${NC}"

# Wait a moment for dev server to be ready
sleep 2

# Platform detection
if [[ -n "$WSL_DISTRO_NAME" ]] || grep -qi microsoft /proc/version 2>/dev/null; then
    # WSL - forward to Windows browser via WSLg
    DISPLAY=:0 xdg-open "$URL" 2>/dev/null &
elif [[ "$OSTYPE" == "darwin"* ]]; then
    # macOS
    open "$URL" 2>/dev/null &
else
    # Linux native
    xdg-open "$URL" 2>/dev/null &
fi

echo -e "${GREEN}✓ Browser opened${NC}"
