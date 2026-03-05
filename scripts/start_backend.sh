#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR/backend"
source "$HOME/.cargo/env"
set -a
source ./.env.local
set +a
cargo run
