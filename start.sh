#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Load cargo into PATH when rustup is installed but shell env is not loaded.
if ! command -v cargo >/dev/null 2>&1; then
  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090
    . "$HOME/.cargo/env"
  fi
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo was not found. Install Rust with: curl https://sh.rustup.rs -sSf | sh -s -- -y" >&2
  exit 1
fi

# Network port (default 9000). Override with: APP_CALL_PORT=9001 ./start.sh
export APP_CALL_PORT="${APP_CALL_PORT:-9000}"

exec cargo run -p desktop "$@"
