#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$ROOT_DIR/.runlogs"

mkdir -p "$LOG_DIR"

start_if_not_running() {
  local pattern="$1"
  local cmd="$2"
  local log_file="$3"
  if pgrep -f "$pattern" >/dev/null 2>&1; then
    echo "[skip] already running: $pattern"
    return 0
  fi
  echo "[start] $cmd"
  nohup bash -lc "cd \"$ROOT_DIR\" && $cmd" >"$log_file" 2>&1 &
  sleep 0.5
}

echo "[mode] dev"
start_if_not_running "target/debug/backend" "./scripts/start_backend.sh" "$LOG_DIR/backend.log"
start_if_not_running "vite --host 0.0.0.0 --port 8080" "./scripts/start_frontend.sh" "$LOG_DIR/frontend.log"

echo
echo "Started (dev mode)."
echo "URL: http://127.0.0.1:8080"
echo "LAN URL: http://<board-ip>:8080"

echo
echo "[logs]"
echo "  $LOG_DIR/backend.log"
echo "  $LOG_DIR/frontend.log"
