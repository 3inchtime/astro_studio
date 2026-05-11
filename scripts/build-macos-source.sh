#!/bin/bash

set -euo pipefail

echo "======================================"
echo "  Astro Studio - macOS Source Build"
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

echo "Building Astro Studio for macOS..."
npm run tauri build

echo "Build finished."
echo "macOS app bundle:"
echo "  $SCRIPT_DIR/src-tauri/target/release/bundle/macos"
echo "DMG bundle:"
echo "  $SCRIPT_DIR/src-tauri/target/release/bundle/dmg"
