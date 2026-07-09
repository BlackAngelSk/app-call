#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "[app-call] Starting auto install..."

if ! command -v cargo >/dev/null 2>&1; then
  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
  fi
fi

if ! command -v cargo >/dev/null 2>&1; then
  if ! command -v curl >/dev/null 2>&1; then
    echo "[app-call] curl is required to install Rust automatically." >&2
    exit 1
  fi

  echo "[app-call] Rust toolchain not found. Installing with rustup..."
  curl https://sh.rustup.rs -sSf | sh -s -- -y

  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
  fi
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "[app-call] cargo still not available after install." >&2
  exit 1
fi

echo "[app-call] Fetching dependencies..."
cargo fetch

echo "[app-call] Building desktop app..."
cargo build -p desktop

echo "[app-call] Install complete. Start with: ./start.sh"
