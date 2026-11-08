#!/usr/bin/env bash
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${BENCH_SHELL_BIN:-$SDIR/../rust/target/release/bench_shell}"
if [ ! -x "$BIN" ]; then
  printf '[bench_shell] ERROR Rust 실행파일이 없거나 실행할 수 없다: %s\n' "$BIN" >&2
  exit 2
fi
exec "$BIN" "$@"
