@echo off
cd /d "%~dp0"
echo Launching Echophrase in Windows native mode...
bun install
bun run tauri:dev