# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Windows Development Launch Script
# Launches Echophrase with GPU+CUDA support on Windows

Write-Host "🚀 Starting Echophrase (Windows + CUDA)" -ForegroundColor Green

# Get script directory and project root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = (Get-Item "$ScriptDir/../..").FullName

# Convert WSL path to Windows path if needed
$WindowsProjectPath = if ($ProjectRoot -like "/mnt/*") {
    # Convert WSL path to Windows path
    $drive = $ProjectRoot.Substring(5,1).ToUpper()
    $path = $ProjectRoot.Substring(6).Replace("/", "\")
    "${drive}:${path}"
} elseif ($ProjectRoot -like "\\wsl*") {
    # Already a Windows UNC path
    $ProjectRoot
} else {
    # Assume it's already a Windows path
    $ProjectRoot
}

# Set location to Windows project directory
Set-Location $WindowsProjectPath

# Set CUDA environment for Windows
$env:NVCC_APPEND_FLAGS = "-allow-unsupported-compiler"
$env:CMAKE_GENERATOR = "Visual Studio 17 2022"
$env:CMAKE_GENERATOR_PLATFORM = "x64"

# Verify CUDA is available
$nvccPath = Get-Command nvcc.exe -ErrorAction SilentlyContinue
if (-not $nvccPath) {
    Write-Host "⚠️  CUDA not found. Checking standard location..." -ForegroundColor Yellow
    $cudaPath = "/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v12.8/bin/nvcc.exe"
    if (Test-Path $cudaPath) {
        $env:PATH = "/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v12.8/bin;$env:PATH"
        Write-Host "✓ CUDA found at standard location" -ForegroundColor Green
    } else {
        Write-Host "❌ CUDA not found. GPU acceleration will not work." -ForegroundColor Red
    }
} else {
    Write-Host "✓ CUDA found: $($nvccPath.Path)" -ForegroundColor Green
}

# Fix PATH to prioritize Visual Studio linker over Git linker and reduce length
Write-Host "🔧 Cleaning and optimizing PATH for NVCC compatibility..." -ForegroundColor Yellow
$originalPath = $env:PATH
$pathParts = $originalPath -split ';'

# Remove Git usr/bin, duplicates, empty entries, and very long paths to fix NVCC vcvars issue
$filteredPath = $pathParts | Where-Object { 
    $_ -and                                    # Remove empty entries
    $_ -notmatch "Git.*usr.*bin" -and         # Remove Git linker conflict
    $_.Length -lt 200 -and                    # Remove very long paths that can cause issues
    $_ -notmatch '""'                         # Remove entries with quotes
} | Select-Object -Unique                     # Remove duplicates

# Ensure essential paths are at the beginning
$essentialPaths = @(
    "C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Tools/MSVC/14.43.34808/bin/Hostx64/x64",
    "C:/Program Files/Microsoft Visual Studio/2022/Community/Common7/IDE",
    "C:/Program Files (x86)/Windows Kits/10/bin/10.0.22621.0/x64",
    "C:/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v12.8/bin",
    "C:/ProgramData/chocolatey/bin",
    "C:/Windows/System32",
    "C:/Windows"
)

$cleanPath = ($essentialPaths + $filteredPath) | Select-Object -Unique
$env:PATH = $cleanPath -join ';'

Write-Host "✓ PATH cleaned and optimized for NVCC" -ForegroundColor Green

# Check for problematic Command Processor AutoRun entries that can break NVCC
try {
    $autoRunValue = Get-ItemProperty -Path "HKLM:\Software\Microsoft\Command Processor" -Name "AutoRun" -ErrorAction SilentlyContinue
    if ($autoRunValue) {
        Write-Host "⚠️  Found Command Processor AutoRun entry that may interfere with NVCC" -ForegroundColor Yellow
        Write-Host "   If build fails, consider clearing HKLM\Software\Microsoft\Command Processor\AutoRun" -ForegroundColor Yellow
    }
} catch {
    # Registry key doesn't exist, which is fine
}

# Check if bun is installed
$bunPath = Get-Command bun.exe -ErrorAction SilentlyContinue
if (-not $bunPath) {
    Write-Host "❌ Bun not found. Please install Bun for Windows." -ForegroundColor Red
    Write-Host "Install from: https://bun.sh/docs/installation#windows" -ForegroundColor Yellow
    exit 1
}

# Kill any existing processes on the dev port (DEV_PORT from env.yaml, default 8420)
$devPort = if ($env:DEV_PORT) { [int]$env:DEV_PORT } else { 8420 }
$portProcess = Get-NetTCPConnection -LocalPort $devPort -ErrorAction SilentlyContinue
if ($portProcess) {
    Write-Host "Killing existing process on port $devPort..." -ForegroundColor Yellow
    Stop-Process -Id $portProcess.OwningProcess -Force -ErrorAction SilentlyContinue
}

# Initialize Visual Studio environment for NVCC before building
Write-Host "🔧 Initializing Visual Studio environment for NVCC..." -ForegroundColor Yellow
$vcvarsPath = "C:/Program Files/Microsoft Visual Studio/2022/Community/VC/Auxiliary/Build/vcvars64.bat"
if (Test-Path $vcvarsPath) {
    Write-Host "✓ Found vcvars64.bat, setting up VS environment" -ForegroundColor Green
    
    # Run vcvars64.bat and capture the environment
    $command = "cmd.exe /c `"$vcvarsPath`" && set"
    $output = Invoke-Expression $command
    
    # Parse and apply environment variables
    foreach ($line in $output) {
        if ($line -match "^([^=]+)=(.*)$") {
            $name = $matches[1]
            $value = $matches[2]
            # Only set important VS-related variables, don't override PATH
            if ($name -match "^(VS|VC|VSINSTALLDIR|VCToolsInstallDir|WindowsSdkDir|INCLUDE|LIB)") {
                Set-Item -Path "env:$name" -Value $value -Force
            }
        }
    }
    Write-Host "✓ Visual Studio environment initialized" -ForegroundColor Green
} else {
    Write-Host "⚠️  vcvars64.bat not found, NVCC may fail" -ForegroundColor Yellow
}

# Launch with environment loading
Write-Host "Loading environment and starting development server..." -ForegroundColor Green
bun run tauri:dev:windows