#!/usr/bin/env bash

# Test script for Candle Whisper transcription with test-1-2.wav
# This script runs the transcription test and shows detailed output

set -e

echo "🧪 Echophrase Transcription Test"
echo "================================"
echo ""

# Set Rust backtrace for debugging
export RUST_BACKTRACE=1

echo "📊 Environment:"
echo "   RUST_BACKTRACE: $RUST_BACKTRACE"
echo ""

# Change to src-tauri directory
cd "$(dirname "$0")"

echo "📂 Working directory: $(pwd)"
echo ""

# Run the test
echo "🚀 Running transcription test..."
echo "   Test: test_real_candle_transcription"
echo "   Audio: ../tests/audio/test-1-2.wav"
echo ""

cargo test test_real_candle_transcription --lib -- --ignored --nocapture

echo ""
echo "✅ Test completed!"
