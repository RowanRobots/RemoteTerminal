#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/backend"
source "$HOME/.cargo/env"
set -a
source ./.env.local
set +a
cargo run
