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

TAURI_PIDS=$(pgrep -f "tauri dev" 2>/dev/null || true)
if [ -n "$TAURI_PIDS" ]; then
    echo "Found running Tauri processes, killing..."
    echo "$TAURI_PIDS" | xargs kill -9 2>/dev/null || true
fi

RUST_PIDS=$(pgrep -f "astro_studio" 2>/dev/null || true)
if [ -n "$RUST_PIDS" ]; then
    echo "Found running Astro Studio processes, killing..."
    echo "$RUST_PIDS" | xargs kill -9 2>/dev/null || true
fi

sleep 1

echo "Starting Astro Studio (Tauri + React dev server)..."
npm run tauri dev
