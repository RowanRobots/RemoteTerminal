#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[1/3] backend cargo test"
(
  cd "$ROOT/backend"
  source "$HOME/.cargo/env"
  cargo test
)

echo "[2/3] frontend build"
(
  cd "$ROOT/frontend"
  npm run build
)

echo "[3/3] frontend test"
(
  cd "$ROOT/frontend"
  npm test
)

echo "All checks passed."
