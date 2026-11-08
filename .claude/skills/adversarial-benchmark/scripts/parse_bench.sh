#!/bin/bash
# Native measurement parser. Explicit override supports isolated integration fixtures.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${PARSE_BENCH_BIN:-$SDIR/../rust/target/release/parse_bench}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[parse_bench] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[parse_bench] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
exec "$BINARY" "$@"
