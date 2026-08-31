#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$SCRIPT_DIR"

log_step() {
  printf '\n====================\n'
  printf '%s\n' "$1"
  printf '====================\n'
}

log_step "Phase 0 quality baseline: frontend checks"
npm run -s build
npm run -s test

log_step "Phase 0 quality baseline: Rust checks"
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets

log_step "Phase 0 quality baseline: tauri artifact sanity"
cd "$SCRIPT_DIR"
npm run -s tauri -- info

echo
echo "Quality entrypoint finished."
