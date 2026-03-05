#!/usr/bin/env bash
set -euo pipefail

kill_by_pattern() {
  local pattern="$1"
  local pids
  pids="$(pgrep -f "$pattern" || true)"
  if [[ -z "$pids" ]]; then
    echo "[skip] no process matched: $pattern"
    return 0
  fi
  echo "[stop] $pattern"
  for pid in $pids; do
    if [[ "$pid" != "$$" ]]; then
      kill "$pid" 2>/dev/null || true
    fi
  done
}

kill_by_pattern "target/debug/backend"
kill_by_pattern "vite --host 0.0.0.0 --port 8080"

sleep 0.5
echo
echo "[ports after stop]"
ss -ltnp | rg '(:8080|:8081)\b' || echo "no listeners on 8080/8081"
