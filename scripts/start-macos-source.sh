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

ensure_brew_available() {
    if command -v brew >/dev/null 2>&1; then
        return
    fi

    echo "Error: Homebrew is required to install missing dependencies."
    echo "Install Homebrew first: https://brew.sh"
    exit 1
}

install_with_brew_if_missing() {
    local command_name="$1"
    local formula="$2"

    if command -v "$command_name" >/dev/null 2>&1; then
        return
    fi

    ensure_brew_available
    echo "'$command_name' is missing. Installing '$formula' with Homebrew..."
    brew install "$formula"
}

install_with_brew_if_missing npm node
install_with_brew_if_missing cargo rust

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
