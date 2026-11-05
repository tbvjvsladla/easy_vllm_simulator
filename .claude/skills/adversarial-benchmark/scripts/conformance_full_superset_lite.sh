#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${CONFORMANCE_BIN:-$SDIR/../rust/target/release/conformance_full_superset_lite}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[conformance_full_superset_lite] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[conformance_full_superset_lite] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
export CONFORMANCE_SCRIPTS_DIR="${CONFORMANCE_SCRIPTS_DIR:-$SDIR}"
exec "$BINARY" "$@"
