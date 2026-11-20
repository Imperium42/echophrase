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

# Check for ONNX Runtime on Windows (ort 2.0.0-rc.11 requires 1.23.2)
$ortPaths = @(
    "$env:USERPROFILE\.local\lib\onnxruntime\onnxruntime-win-x64-1.23.2\lib\onnxruntime.dll",
    "$env:USERPROFILE\.local\lib\onnxruntime\onnxruntime-win-x64-gpu-1.23.2\lib\onnxruntime.dll",
    "C:\Program Files\onnxruntime\lib\onnxruntime.dll",
    "$env:LOCALAPPDATA\onnxruntime\onnxruntime.dll"
)

$foundOrt = $null
foreach ($path in $ortPaths) {
    if (Test-Path $path) {
        $foundOrt = $path
        break
    }
}

if ($foundOrt) {
    Write-Host "Found ONNX Runtime: $foundOrt" -ForegroundColor Green
    $env:ORT_DYLIB_PATH = $foundOrt
} else {
    Write-Host "ONNX Runtime not found. Downloading Windows GPU version..." -ForegroundColor Yellow

    $ortDir = "$env:USERPROFILE\.local\lib\onnxruntime"
    New-Item -ItemType Directory -Force -Path $ortDir | Out-Null

    # Windows DirectML build via NuGet (github.com/microsoft/onnxruntime went 404
    # on 2026-11-20; NuGet is the same source the release workflow uses).
    $ortUrl = "https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/1.23.2"
    $ortZip = "$ortDir\ort-directml-1.23.2.zip"

    Write-Host "Downloading from $ortUrl..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $ortUrl -OutFile $ortZip -UseBasicParsing

    Write-Host "Extracting..." -ForegroundColor Cyan
    $extract = "$ortDir\ort-directml-1.23.2"
    Expand-Archive -Path $ortZip -DestinationPath $extract -Force
    Remove-Item $ortZip -Force

    $env:ORT_DYLIB_PATH = "$extract\runtimes\win-x64\native\onnxruntime.dll"
    Write-Host "ONNX Runtime installed: $env:ORT_DYLIB_PATH" -ForegroundColor Green
}

# Add ONNX Runtime to PATH
$ortLibDir = Split-Path $env:ORT_DYLIB_PATH
$env:PATH = "$ortLibDir;$env:PATH"

Write-Host ""
Write-Host "Running transcription test..." -ForegroundColor Cyan
Write-Host "Expected output: 'test 1 2' or similar" -ForegroundColor Yellow
Write-Host ""

# Run the test
cargo test test_transcribe_audio_file --lib -- --nocapture

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "TEST PASSED!" -ForegroundColor Green
} else {
    Write-Host ""
    Write-Host "TEST FAILED (exit code: $LASTEXITCODE)" -ForegroundColor Red
}
