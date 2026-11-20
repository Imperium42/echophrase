# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Windows Unit Test Script
# Runs pure Rust unit tests (no ONNX DLLs needed)
# Usage: bun run test:rust:unit:win

# Use Windows temp dir for build artifacts (avoids WSL filesystem issues)
$env:CARGO_INCREMENTAL = "0"
$env:CARGO_TARGET_DIR = "C:\temp\echophrase-target"

Write-Host "Running Rust unit tests (Windows)..." -ForegroundColor Cyan
Write-Host "Target dir: $env:CARGO_TARGET_DIR" -ForegroundColor DarkGray

Set-Location "$PSScriptRoot\..\..\src-tauri"

cargo test -- test_resample test_execution test_normalize test_model_type

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nAll unit tests passed!" -ForegroundColor Green
} else {
    Write-Host "`nSome tests failed (exit code: $LASTEXITCODE)" -ForegroundColor Red
    exit $LASTEXITCODE
}
