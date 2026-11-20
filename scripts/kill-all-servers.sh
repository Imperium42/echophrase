#!/bin/bash

# SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
#
# SPDX-License-Identifier: MPL-2.0

# Dev server port (DEV_PORT from env.yaml, default 8420)
DEV_PORT="${DEV_PORT:-8420}"

echo "🔪 Killing ALL dev servers and processes..."

# Kill all processes on port ${DEV_PORT}
echo "Killing processes on port ${DEV_PORT}..."
lsof -ti :${DEV_PORT} 2>/dev/null | while read pid; do
    echo "  Killing PID $pid on port ${DEV_PORT}"
    kill -9 $pid 2>/dev/null
done

# Kill all Vite processes
echo "Killing all Vite processes..."
pkill -9 -f vite 2>/dev/null

# Kill all node processes related to the project
echo "Killing all Node processes in echophrase..."
pkill -9 -f "node.*echophrase" 2>/dev/null

# Kill all bun processes
echo "Killing all Bun processes..."
pkill -9 -f bun 2>/dev/null

# Kill Tauri dev processes
echo "Killing Tauri dev processes..."
pkill -9 -f "tauri dev" 2>/dev/null

# Kill cargo run processes
echo "Killing cargo processes..."
pkill -9 -f "cargo.*run" 2>/dev/null

# Kill the Tauri app itself
echo "Killing echophrase binary..."
pkill -9 echophrase 2>/dev/null

# Double check port ${DEV_PORT}
sleep 1
if lsof -ti :${DEV_PORT} >/dev/null 2>&1; then
    echo "⚠️  Port ${DEV_PORT} still in use, force killing..."
    for pid in $(lsof -ti :${DEV_PORT}); do
        echo "  Force killing PID $pid"
        kill -9 $pid 2>/dev/null || sudo kill -9 $pid 2>/dev/null
    done
fi

# Clear Vite cache
echo "Clearing Vite cache..."
rm -rf node_modules/.vite 2>/dev/null

echo "✅ All servers should be killed. Checking port ${DEV_PORT}..."
if lsof -ti :${DEV_PORT} >/dev/null 2>&1; then
    echo "❌ WARNING: Port ${DEV_PORT} is STILL in use by:"
    lsof -i :${DEV_PORT}
    echo ""
    echo "You may need to run: sudo lsof -ti :${DEV_PORT} | xargs sudo kill -9"
else
    echo "✅ Port ${DEV_PORT} is free!"
fi

# Kill Windows processes on port ${DEV_PORT} (for WSL users)
echo "Checking Windows side for port ${DEV_PORT}..."
if command -v pwsh.exe &> /dev/null; then
    # Get Windows PIDs listening on port ${DEV_PORT}
    WINDOWS_PIDS=$(pwsh.exe -c "netstat -ano | findstr :${DEV_PORT} | findstr LISTENING" 2>/dev/null | awk '{print $5}' | sort -u)
    
    if [ ! -z "$WINDOWS_PIDS" ]; then
        echo "Found Windows processes on port ${DEV_PORT}:"
        for pid in $WINDOWS_PIDS; do
            echo "  Killing Windows PID $pid"
            pwsh.exe -c "taskkill /F /PID $pid" 2>/dev/null
        done
    fi
fi

echo ""
echo "🚀 You can now start fresh with: bun run dev"