REM SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
REM
REM SPDX-License-Identifier: MPL-2.0

@echo off
cd /d "%~dp0"
echo Launching Echophrase in Windows native mode...
bun install
bun run tauri:dev