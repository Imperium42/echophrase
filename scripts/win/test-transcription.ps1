# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Windows Transcription Test Script
# Tests speech-to-text with DirectML (GPU) execution provider
# Usage: pwsh.exe scripts/win/test-transcription.ps1
# Works from both WSL filesystem and native Windows paths

Write-Host "Testing Echophrase Transcription (Windows DirectML)" -ForegroundColor Green

# Get script directory and project root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Get-Item "$ScriptDir/../..").FullName

# Detect if running from WSL filesystem
$IsWslFs = $ProjectRoot -like "\\wsl*" -or $ProjectRoot -like "//wsl*"
if ($IsWslFs) {
    Write-Host "Running from WSL filesystem - using Windows target dir" -ForegroundColor Yellow
}

Set-Location "$ProjectRoot\src-tauri"
Write-Host "Working directory: $(Get-Location)" -ForegroundColor Cyan

# Disable incremental compilation and use Windows target dir (avoids WSL lock file issues)
$env:CARGO_INCREMENTAL = "0"
$env:CARGO_TARGET_DIR = "C:\temp\echophrase-target"
Write-Host "Target directory: $env:CARGO_TARGET_DIR" -ForegroundColor DarkGray

# Enable ONNX Runtime verbose logging to verify actual execution provider
$env:ORT_LOG_LEVEL = "verbose"

# ONNX Runtime (ort 2.0.0-rc.12 requires 1.24.x; a 1.23 DLL crashes the test with
# STATUS_ENTRYPOINT_NOT_FOUND). An ORT_DYLIB_PATH already in the environment wins;
# otherwise use the pinned runtime scripts/ensure-ort.mjs stages.
if (-not ($env:ORT_DYLIB_PATH -and (Test-Path $env:ORT_DYLIB_PATH))) {
    # Absolute paths: the working directory is src-tauri at this point.
    bun "$ProjectRoot/scripts/ensure-ort.mjs" --ci
    if ($LASTEXITCODE -ne 0) { throw "ensure-ort.mjs failed" }
    $env:ORT_DYLIB_PATH = (Resolve-Path "$ProjectRoot/src-tauri/resources/ort/directml/onnxruntime.dll").Path
}
Write-Host "ONNX Runtime: $env:ORT_DYLIB_PATH" -ForegroundColor Green

# Add ONNX Runtime to PATH
$ortLibDir = Split-Path $env:ORT_DYLIB_PATH
$env:PATH = "$ortLibDir;$env:PATH"

Write-Host ""
Write-Host "Running transcription test..." -ForegroundColor Cyan
Write-Host "Expected output: 'test 1 2' or similar" -ForegroundColor Yellow
Write-Host ""

# Run the test. It is #[ignore]d (needs ORT + a model), so --ignored is
# required - without it 0 tests ran and this script still printed PASSED.
# Collect lines while streaming them so the result line can be checked.
$testLog = [System.Collections.Generic.List[string]]::new()
cargo test test_transcribe_audio_file --lib -- --ignored --nocapture 2>&1 | ForEach-Object {
    $line = "$_"
    $testLog.Add($line)
    Write-Host $line
}
$code = $LASTEXITCODE
$ran = ($testLog -join "`n") -match 'test result: ok\. 1 passed'

if ($code -eq 0 -and $ran) {
    Write-Host ""
    Write-Host "TEST PASSED!" -ForegroundColor Green
} elseif ($code -eq 0) {
    Write-Host ""
    Write-Host "TEST FAILED: the transcription test did not run (0 passed)" -ForegroundColor Red
    exit 1
} else {
    Write-Host ""
    Write-Host "TEST FAILED (exit code: $code)" -ForegroundColor Red
    exit $code
}
