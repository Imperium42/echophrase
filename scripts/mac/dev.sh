#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Dev server port (DEV_PORT from env.yaml, default 8420)
DEV_PORT="${DEV_PORT:-8420}"
# macOS Development Launch Script
# Launches Echophrase with Metal GPU (or CPU fallback) on macOS

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🍎 Starting Echophrase (macOS - Metal/CPU)${NC}"

# Navigate to project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$PROJECT_ROOT"

# ── Pre-flight checks ─────────────────────────────────────────────────────

# Check architecture (Apple Silicon required)
ARCH=$(uname -m)
if [ "$ARCH" != "arm64" ]; then
    echo -e "${RED}Error: Apple Silicon (arm64) required, detected: ${ARCH}${NC}"
    echo -e "${RED}Intel Macs are not supported - Metal GPU requires Apple Silicon.${NC}"
    exit 1
fi

# Check macOS version >= 13.3 (required for ONNX Runtime 1.24.x)
MACOS_VERSION=$(sw_vers -productVersion)
MAJOR=$(echo "$MACOS_VERSION" | cut -d. -f1)
MINOR=$(echo "$MACOS_VERSION" | cut -d. -f2)
if [ "$MAJOR" -lt 13 ] || { [ "$MAJOR" -eq 13 ] && [ "${MINOR:-0}" -lt 3 ]; }; then
    echo -e "${RED}Error: macOS 13.3+ required for ONNX Runtime, detected: ${MACOS_VERSION}${NC}"
    echo -e "${YELLOW}Candle Metal backend works on macOS 13.0+, but ONNX models (Parakeet) need 13.3+.${NC}"
    exit 1
fi
echo -e "${GREEN}macOS ${MACOS_VERSION} (${ARCH}) - OK${NC}"

# Check ONNX Runtime dylib
if [ -n "$ORT_DYLIB_PATH" ]; then
    if [ ! -f "$ORT_DYLIB_PATH" ]; then
        echo -e "${YELLOW}Warning: ORT_DYLIB_PATH set but file not found: ${ORT_DYLIB_PATH}${NC}"
        echo -e "${YELLOW}Run: bun run setup:ort${NC}"
    else
        echo -e "${GREEN}ONNX Runtime: ${ORT_DYLIB_PATH} - OK${NC}"
    fi
else
    echo -e "${YELLOW}ORT_DYLIB_PATH not set - ONNX models will be unavailable.${NC}"
    echo -e "${YELLOW}Run: bun run setup:ort${NC}"
fi

# ── Launch ─────────────────────────────────────────────────────────────────

# Kill any existing processes on port ${DEV_PORT}
if lsof -ti:${DEV_PORT} >/dev/null 2>&1; then
    echo -e "${YELLOW}Killing existing process on port ${DEV_PORT}...${NC}"
    lsof -ti:${DEV_PORT} | xargs kill -9 2>/dev/null || true
fi

# Launch with environment loading
echo -e "${GREEN}Loading environment and starting development server...${NC}"
bun run tauri:dev:mac
