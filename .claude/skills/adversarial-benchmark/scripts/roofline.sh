#!/bin/bash
# Native owner command. Node-local build is required before execution.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${ROOFLINE_BIN:-$SDIR/../rust/target/release/roofline}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[roofline] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[roofline] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
exec "$BINARY" "$@"
