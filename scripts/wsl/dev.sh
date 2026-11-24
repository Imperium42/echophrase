#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Dev server port (DEV_PORT from env.yaml, default 8420)
DEV_PORT="${DEV_PORT:-8420}"
# WSL Development Launch Script
# Launches Echophrase in WSL (CPU-only; see https://docs.echophrase.com/developers/decisions/0001-no-cuda)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Starting Echophrase (WSL)${NC}"

# Navigate to project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$PROJECT_ROOT"

# Kill any existing processes on port ${DEV_PORT}
if lsof -ti:${DEV_PORT} >/dev/null 2>&1; then
    echo -e "${YELLOW}Killing existing process on port ${DEV_PORT}...${NC}"
    lsof -ti:${DEV_PORT} | xargs kill -9 2>/dev/null || true
fi

# Launch with environment loading
echo -e "${GREEN}Loading environment and starting development server...${NC}"

# Launch browser in background after giving server time to start
"$SCRIPT_DIR/../open-browser.sh" &

bun run tauri:dev