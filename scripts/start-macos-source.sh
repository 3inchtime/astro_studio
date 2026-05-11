#!/bin/bash

set -euo pipefail

echo "======================================"
echo "  Astro Studio - macOS Source Start"
echo "======================================"

if [ "$(uname -s)" != "Darwin" ]; then
    echo "Error: this script is intended for macOS only."
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$SCRIPT_DIR"

if ! command -v npm >/dev/null 2>&1; then
    echo "Error: npm is not installed. Install Node.js 22+ and npm 11+ first."
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: cargo is not installed. Install the Rust stable toolchain first."
    exit 1
fi

echo "Installing frontend dependencies..."
npm install

echo "Fetching Rust dependencies..."
cargo fetch --manifest-path src-tauri/Cargo.toml

echo "Checking for existing development processes..."
if lsof -ti:1420 >/dev/null 2>&1; then
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

echo "Starting Astro Studio from source (Tauri + React dev server)..."
npm run tauri dev
