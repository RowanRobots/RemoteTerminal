#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_HOME="${TARGET_HOME:-$HOME}"
INSTALL_ROOT="${TTYD_INSTALL_ROOT:-$TARGET_HOME/.local/opt/ttyd}"
PKG_ROOT="$INSTALL_ROOT/pkg"
LIB_DIR="$PKG_ROOT/usr/lib/aarch64-linux-gnu"
BIN_DIR="$TARGET_HOME/.local/bin"
TMP_DIR="${TMPDIR:-/tmp}/remoteterminal-ttyd"

mkdir -p "$PKG_ROOT" "$BIN_DIR" "$TMP_DIR"

if command -v ttyd >/dev/null 2>&1; then
  echo "[skip] ttyd already exists in PATH: $(command -v ttyd)"
  exit 0
fi

if ! command -v apt >/dev/null 2>&1; then
  echo "apt is required to download ttyd packages automatically" >&2
  exit 1
fi

echo "[download] ttyd packages"
(
  cd "$TMP_DIR"
  rm -f ttyd_*_arm64.deb libev4_*_arm64.deb
  apt download ttyd libev4 >/dev/null
)

echo "[extract] ttyd into $PKG_ROOT"
dpkg-deb -x "$TMP_DIR"/ttyd_*_arm64.deb "$PKG_ROOT"
dpkg-deb -x "$TMP_DIR"/libev4_*_arm64.deb "$PKG_ROOT"

cat >"$BIN_DIR/ttyd" <<EOF
#!/usr/bin/env bash
set -euo pipefail
export LD_LIBRARY_PATH="$LIB_DIR\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
exec "$PKG_ROOT/usr/bin/ttyd" "\$@"
EOF

chmod +x "$BIN_DIR/ttyd"

echo "[ok] ttyd installed"
echo "binary: $PKG_ROOT/usr/bin/ttyd"
echo "wrapper: $BIN_DIR/ttyd"
