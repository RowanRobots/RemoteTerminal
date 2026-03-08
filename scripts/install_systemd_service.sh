#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_NAME="${SERVICE_NAME:-remoteterminal}"
INSTALL_MODE="system"

usage() {
  cat <<'EOF'
Usage: scripts/install_systemd_service.sh [--user]

Options:
  --user      Install as a user service instead of a system service.

This script expects scripts/publish_runtime.sh to have already rendered:
- deploy/systemd/remoteterminal.service.rendered
- .prod-runtime/current/*
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --user)
      INSTALL_MODE="user"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

APP_USER="${SUDO_USER:-${USER:-$(id -un)}}"
APP_HOME="$(getent passwd "$APP_USER" | cut -d: -f6)"
UNIT_RENDERED="$ROOT_DIR/deploy/systemd/${SERVICE_NAME}.service.rendered"

if [[ -z "$APP_HOME" ]]; then
  echo "failed to resolve home directory for $APP_USER" >&2
  exit 1
fi

if [[ ! -f "$UNIT_RENDERED" ]]; then
  echo "missing rendered unit: $UNIT_RENDERED" >&2
  echo "run ./scripts/publish_runtime.sh first" >&2
  exit 1
fi

if [[ "$INSTALL_MODE" == "user" ]]; then
  USER_UNIT_DIR="$APP_HOME/.config/systemd/user"
  mkdir -p "$USER_UNIT_DIR"
  install -m 0644 "$UNIT_RENDERED" "$USER_UNIT_DIR/$SERVICE_NAME.service"
  systemctl --user daemon-reload
  systemctl --user enable --now "$SERVICE_NAME.service"
  echo "[ok] user service installed: $SERVICE_NAME.service"
  exit 0
fi

sudo install -m 0644 "$UNIT_RENDERED" "/etc/systemd/system/$SERVICE_NAME.service"
sudo systemctl daemon-reload
sudo systemctl enable --now "$SERVICE_NAME.service"

echo "[ok] system service installed: $SERVICE_NAME.service"
