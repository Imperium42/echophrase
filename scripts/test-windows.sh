#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Windows test runner - DirectML transcription + environment integration checks.
#
# Flags:
#   (no flag)       Run DirectML transcription test via PowerShell.
#                   Requires WIN_ECHOPHRASE_PATH to be set (native Windows path).
#   --integration   Run environment/platform detection checks only (no GPU required).
#                   Checks WSL detection, CUDA, cuDNN, env vars, and model cache.
#
# Examples:
#   export WIN_ECHOPHRASE_PATH='C:\dev\echophrase'
#   bash scripts/test-windows.sh                  # transcription test
#   bash scripts/test-windows.sh --integration    # environment checks

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# ── Helpers ──────────────────────────────────────────────────────────────────

is_wsl() {
    if [[ -f /proc/version ]]; then
        if grep -qi "microsoft\|wsl" /proc/version; then
            return 0
        fi
    fi
    if [[ -n "$WSL_DISTRO_NAME" ]] || [[ -n "$WSL_INTEROP" ]]; then
        return 0
    fi
    return 1
}

# ── Mode: integration checks ─────────────────────────────────────────────────

run_integration() {
    echo "Echophrase Windows/WSL Integration Test"
    echo "=========================================="

    echo -e "\nPlatform Detection:"
    if is_wsl; then
        echo "   WSL environment detected"
        echo "   Distribution: $WSL_DISTRO_NAME"

        if [[ -f /usr/lib/wsl/lib/libcuda.so ]]; then
            echo "   WSL GPU passthrough: Available"
        else
            echo "   Warning: WSL GPU passthrough not configured"
        fi

        echo -e "\nWindows Interop Test:"
        if command -v pwsh.exe &> /dev/null; then
            echo "   PowerShell accessible from WSL"

            WINDOWS_CUDA=$(pwsh.exe -Command 'if (Test-Path "$env:CUDA_PATH") { Write-Host "Found: $env:CUDA_PATH" } else { Write-Host "Not found" }' 2>/dev/null | tr -d '\r')
            echo "   Windows CUDA: $WINDOWS_CUDA"

            WINDOWS_GPU=$(pwsh.exe -Command 'try { $gpu = Get-WmiObject Win32_VideoController | Where-Object { $_.Name -like "*NVIDIA*" } | Select-Object -First 1; if ($gpu) { Write-Host $gpu.Name } else { Write-Host "No NVIDIA GPU" } } catch { Write-Host "Error detecting GPU" }' 2>/dev/null | tr -d '\r')
            echo "   Windows GPU: $WINDOWS_GPU"
        else
            echo "   Warning: PowerShell not accessible from WSL"
        fi
    else
        echo "   Native Linux environment"
    fi

    echo -e "\nCUDA Detection (WSL/Linux):"
    if [[ -d /usr/local/cuda ]]; then
        echo "   CUDA found at /usr/local/cuda"
        if [[ -x /usr/local/cuda/bin/nvcc ]]; then
            CUDA_VERSION=$(/usr/local/cuda/bin/nvcc --version | grep "release" | sed 's/.*release //' | sed 's/,.*//')
            echo "   CUDA version: $CUDA_VERSION"
        fi
    else
        echo "   Warning: CUDA not found at /usr/local/cuda"
    fi

    echo -e "\ncuDNN Detection:"
    if ls /usr/lib/x86_64-linux-gnu/libcudnn*.so* 2>/dev/null | head -1 > /dev/null; then
        echo "   cuDNN found in /usr/lib/x86_64-linux-gnu/"
        CUDNN_FILE=$(ls /usr/lib/x86_64-linux-gnu/libcudnn*.so* 2>/dev/null | head -1)
        echo "   cuDNN library: $(basename "$CUDNN_FILE")"
    elif ls /usr/local/cuda/lib64/libcudnn*.so* 2>/dev/null | head -1 > /dev/null; then
        echo "   cuDNN found in CUDA directory"
    else
        echo "   Warning: cuDNN not found"
    fi

    echo -e "\nEnvironment Variables:"
    env_vars=("CUDA_HOME" "CUDA_PATH" "CUDNN_PATH" "CT2_CUDA_TRUE_FP16_GEMM" "LD_LIBRARY_PATH" "PULSE_SERVER")
    for var in "${env_vars[@]}"; do
        value="${!var}"
        if [[ -n "$value" ]]; then
            if [[ ${#value} -gt 60 ]]; then
                display_value="${value:0:57}..."
            else
                display_value="$value"
            fi
            echo "   $var = $display_value"
        else
            echo "   Warning: $var not set"
        fi
    done

    echo -e "\nModel Cache Detection:"
    CACHE_DIR="$HOME/.cache/huggingface/hub"
    if [[ -d "$CACHE_DIR" ]]; then
        echo "   Cache directory exists: $CACHE_DIR"

        MODEL_DIR="$CACHE_DIR/models--Zoont--faster-whisper-large-v3-turbo-int8-ct2"
        if [[ -d "$MODEL_DIR" ]]; then
            echo "   Whisper INT8 model found in cache"
            if [[ -f "$MODEL_DIR/snapshots/main/model.bin" ]]; then
                MODEL_SIZE=$(du -h "$MODEL_DIR/snapshots/main/model.bin" | cut -f1)
                echo "   Model size: $MODEL_SIZE"
            fi
        else
            echo "   Warning: Whisper INT8 model not in cache"
        fi
    else
        echo "   Warning: cache directory not found"
    fi

    echo -e "\nIntegration test completed!"
    echo ""
    echo "Summary:"
    if is_wsl; then
        echo "   - Running in WSL - can access both Windows and Linux CUDA"
        echo "   - Use env.yaml wsl section for WSL-specific configuration"
        echo "   - PulseAudio configured for audio: PULSE_SERVER=$PULSE_SERVER"
    else
        echo "   - Running in native Linux"
        echo "   - Use standard Linux CUDA configuration"
    fi
}

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

# ── Dispatch ──────────────────────────────────────────────────────────────────

if [[ "${1:-}" == "--integration" ]]; then
    run_integration
else
    run_transcription
fi
