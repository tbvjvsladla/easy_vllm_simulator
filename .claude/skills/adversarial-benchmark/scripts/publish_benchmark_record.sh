#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${PUBLISH_BENCHMARK_RECORD_BIN:-$SDIR/../rust/target/release/publish_benchmark_record}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[publish_benchmark_record] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[publish_benchmark_record] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
exec "$BINARY" "$@"
