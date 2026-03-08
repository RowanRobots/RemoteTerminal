#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR/backend"
source "$HOME/.cargo/env"
export PATH="$HOME/.local/opt/ttyd/pkg/usr/bin:$PATH"
export LD_LIBRARY_PATH="$HOME/.local/opt/ttyd/pkg/usr/lib/aarch64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
set -a
source ./.env.local
set +a
cargo run
