#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_ENV_FILE="${REMOTE_TERMINAL_ENV_FILE:-$ROOT_DIR/deploy/systemd/remoteterminal.env.local}"

if [[ -f "$SERVICE_ENV_FILE" ]]; then
  set -a
  # shellcheck disable=SC1090
  source "$SERVICE_ENV_FILE"
  set +a
fi

if [[ -z "${HOME:-}" ]]; then
  echo "HOME is not set" >&2
  exit 1
fi

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

export PATH="$HOME/.local/bin:$PATH"

if [[ -n "${CODEX_BIN_DIR:-}" ]]; then
  export PATH="$CODEX_BIN_DIR:$PATH"
fi

if [[ -n "${TTYD_ROOT:-}" ]]; then
  export PATH="$TTYD_ROOT/usr/bin:$PATH"
  export LD_LIBRARY_PATH="$TTYD_ROOT/usr/lib/aarch64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

mkdir -p "$ROOT_DIR/.runlogs"

if [[ -z "${FRONTEND_DIST:-}" ]]; then
  export FRONTEND_DIST="$ROOT_DIR/frontend/dist"
fi

if [[ ! -f "$FRONTEND_DIST/index.html" ]]; then
  echo "frontend dist is missing: $FRONTEND_DIST/index.html" >&2
  echo "run scripts/install_systemd_service.sh to build the production assets" >&2
  exit 1
fi

cd "$ROOT_DIR/backend"
exec "$ROOT_DIR/backend/target/debug/backend"
