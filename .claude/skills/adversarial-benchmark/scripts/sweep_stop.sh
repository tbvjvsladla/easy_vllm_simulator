#!/bin/bash
# Native entry point. SWEEP_STOP_BIN explicitly selects an already built executable.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${SWEEP_STOP_BIN:-$SDIR/../rust/target/release/sweep_stop}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[sweep_stop] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[sweep_stop] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
exec "$BINARY" "$@"
