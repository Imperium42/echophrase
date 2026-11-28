#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Test Transcription Script
# Runs the Rust transcription tests with proper environment setup

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}🧪 Running Transcription Tests${NC}"
echo "================================"

# Environment for the test run
export RUST_BACKTRACE=1
export RUST_LOG=info

# Navigate to src-tauri directory
cd "$(dirname "$0")/../src-tauri" || exit 1

echo -e "${GREEN}📂 Working directory:${NC} $(pwd)"
echo -e "${GREEN}🎯 Test file:${NC} ../tests/audio/test-1-2.wav"
echo ""

# Run the test with a timeout
echo -e "${YELLOW}Running test...${NC}"
if timeout 30 cargo test test_transcribe_audio_file --lib -- --nocapture; then
    echo ""
    echo -e "${GREEN}✅ Test passed successfully!${NC}"
    exit 0
else
    exit_code=$?
    if [ $exit_code -eq 124 ]; then
        echo ""
        echo -e "${RED}❌ Test timed out after 30 seconds${NC}"
        echo "This might indicate the model is still using float32 conversion"
    else
        echo ""
        echo -e "${RED}❌ Test failed with exit code: $exit_code${NC}"
    fi
    exit $exit_code
fi