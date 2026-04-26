#!/bin/bash

set -e

echo "======================================"
echo "  Astro Studio - Starting..."
echo "======================================"

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$SCRIPT_DIR"

if ! command -v npm &> /dev/null; then
    echo "Error: npm is not installed"
    exit 1
fi

if [ ! -d "node_modules" ]; then
    echo "Installing dependencies..."
    npm install
fi

echo "Checking for existing processes..."
if lsof -ti:1420 > /dev/null 2>&1; then
    echo "Found process on port 1420, killing..."
    lsof -ti:1420 | xargs kill -9 2>/dev/null || true
fi

for PATTERN in \
    "tauri dev" \
    "cargo run --no-default-features" \
    "target/debug/astro-studio" \
    "/Applications/Astro Studio.app/Contents/MacOS/astro-studio" \
    "Astro Studio.app/Contents/MacOS/astro-studio" \
    "astro-studio"
do
    MATCHING_PIDS=$(pgrep -f "$PATTERN" 2>/dev/null || true)
    if [ -n "$MATCHING_PIDS" ]; then
        echo "Found processes matching '$PATTERN', killing..."
        echo "$MATCHING_PIDS" | xargs kill -9 2>/dev/null || true
    fi
done

sleep 1

echo "Starting Astro Studio (Tauri + React dev server)..."
npm run tauri dev
