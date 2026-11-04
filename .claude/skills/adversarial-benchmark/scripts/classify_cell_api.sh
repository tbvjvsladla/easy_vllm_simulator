#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${CLASSIFY_CELL_API_BIN:-$SDIR/../rust/target/release/classify_cell_api}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[classify_cell] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[classify_cell] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
export CLASSIFY_CELL_SCRIPTS_DIR="${CLASSIFY_CELL_SCRIPTS_DIR:-$SDIR}"
export CLASSIFY_CELL_ROLE_ADAPTER="${CLASSIFY_CELL_ROLE_ADAPTER:-$SDIR/classify_cell_role_query.py}"
exec "$BINARY" "$@"
