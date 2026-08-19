#!/usr/bin/env bash
#
# DBX one-click launcher for HarmonyOS local build.
#
# Usage:
#   ./start-dbx.sh [web]            Start dbx-web (default)
#   ./start-dbx.sh cli <args...>    Run dbx CLI
#   ./start-dbx.sh build            Build dbx-web and dbx CLI, then start web
#
# Environment overrides:
#   DBX_PORT           Web server port (default: 4224)
#   DBX_STATIC_DIR     Frontend dist directory (default: .portable/dist)
#   DBX_DATA_DIR       Data directory (default: .portable/data)
#   DBX_DISABLE_PASSWORD  Password protection toggle (default: 1)
#
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_BIN="$PROJECT_ROOT/target/release/dbx-web"
CLI_BIN="$PROJECT_ROOT/target/release/dbx"
PORTABLE_DIR="$PROJECT_ROOT/.portable"

DIST_DIR="${DBX_STATIC_DIR:-$PORTABLE_DIR/dist}"
DATA_DIR="${DBX_DATA_DIR:-$PORTABLE_DIR/data}"
PORT="${DBX_PORT:-4224}"
DISABLE_PASSWORD="${DBX_DISABLE_PASSWORD:-1}"

MODE="${1:-web}"

usage() {
  echo "Usage: $0 [web|cli|build]" >&2
  echo "  web            Start dbx-web (default)" >&2
  echo "  cli [args...]  Run dbx CLI" >&2
  echo "  build          Build dbx-web/dbx, then start web" >&2
  exit 1
}

build_all() {
  echo "==> Building dbx-web ..."
  (cd "$PROJECT_ROOT" && cargo build --release -p dbx-web --no-default-features --features duckdb-sidecar,mq-admin)
  echo "==> Building dbx CLI ..."
  (cd "$PROJECT_ROOT" && cargo build --release -p dbx-cli --no-default-features)
}

case "$MODE" in
  web|--web)
    if [ ! -x "$WEB_BIN" ]; then
      echo "==> dbx-web not found, building first ..."
      (cd "$PROJECT_ROOT" && cargo build --release -p dbx-web --no-default-features --features duckdb-sidecar,mq-admin)
    fi
    if [ ! -d "$DIST_DIR" ]; then
      echo "ERROR: frontend dist not found: $DIST_DIR" >&2
      echo "Put DBX frontend dist into $PORTABLE_DIR/dist or set DBX_STATIC_DIR." >&2
      exit 1
    fi
    mkdir -p "$DATA_DIR"
    echo "==> Starting dbx-web on http://127.0.0.1:$PORT"
    exec env \
      DBX_STATIC_DIR="$DIST_DIR" \
      DBX_DATA_DIR="$DATA_DIR" \
      DBX_DISABLE_PASSWORD="$DISABLE_PASSWORD" \
      DBX_PORT="$PORT" \
      "$WEB_BIN"
    ;;

  cli|--cli)
    shift || true
    if [ ! -x "$CLI_BIN" ]; then
      echo "==> dbx CLI not found, building first ..."
      (cd "$PROJECT_ROOT" && cargo build --release -p dbx-cli --no-default-features)
    fi
    exec "$CLI_BIN" "$@"
    ;;

  build)
    build_all
    echo "==> Build finished. Starting dbx-web ..."
    exec env \
      DBX_STATIC_DIR="$DIST_DIR" \
      DBX_DATA_DIR="$DATA_DIR" \
      DBX_DISABLE_PASSWORD="$DISABLE_PASSWORD" \
      DBX_PORT="$PORT" \
      "$WEB_BIN"
    ;;

  -h|--help|help)
    sed -n '2,12p' "$0"
    exit 0
    ;;

  *)
    usage
    ;;
esac
