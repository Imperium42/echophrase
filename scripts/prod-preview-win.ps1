# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Production Preview Build Script
# Builds and runs Echophrase with signing keys for update testing

param(
    [switch]$BuildOnly,
    [switch]$RunOnly
)

$ErrorActionPreference = "Stop"

# Load environment variables from .env
$envFile = Join-Path $PSScriptRoot "../.env"
if (Test-Path $envFile) {
    Get-Content $envFile | ForEach-Object {
        if ($_ -match "^\s*([^#][^=]+)=(.*)$") {
            $name = $matches[1].Trim()
            $value = $matches[2].Trim()
            [Environment]::SetEnvironmentVariable($name, $value, "Process")
        }
    }
    Write-Host "Loaded environment from .env" -ForegroundColor Green
} else {
    Write-Host "Warning: .env file not found at $envFile" -ForegroundColor Yellow
}

# Verify signing keys are set
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    Write-Host "Error: TAURI_SIGNING_PRIVATE_KEY not set" -ForegroundColor Red
    exit 1
}

$exePath = Join-Path $PSScriptRoot "../src-tauri/target/release/Echophrase.exe"

if (-not $RunOnly) {
    Write-Host "`nBuilding production build with signing..." -ForegroundColor Cyan
    Push-Location (Join-Path $PSScriptRoot "..")
    try {
        bun run tauri:build
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Build failed!" -ForegroundColor Red
            exit 1
        }
    } finally {
        Pop-Location
    }
    Write-Host "`nBuild complete!" -ForegroundColor Green
    Write-Host "Installers at: src-tauri/target/release/bundle/" -ForegroundColor Gray
}

if (-not $BuildOnly) {
    if (-not (Test-Path $exePath)) {
        Write-Host "Error: Executable not found. Run without -RunOnly first." -ForegroundColor Red
        exit 1
    }
    Write-Host "`nLaunching Echophrase..." -ForegroundColor Cyan
    & $exePath
}
