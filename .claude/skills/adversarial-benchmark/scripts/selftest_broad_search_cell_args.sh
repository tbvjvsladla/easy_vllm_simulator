#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SELF_DIR="${BASH_SOURCE[0]%/*}"
if [ "$SELF_DIR" = "${BASH_SOURCE[0]}" ]; then SELF_DIR=.; fi
SDIR="$(cd -- "$SELF_DIR" && pwd)"
export BENCH_VERIFY_REPO_ROOT="${BENCH_VERIFY_REPO_ROOT:-$(cd "$SDIR/../../../.." && pwd)}"
BINARY="${SELFTEST_BROAD_CELL_BIN:-$SDIR/../rust/target/release/selftest_broad_search_cell_args}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[selftest_broad_search_cell_args] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[selftest_broad_search_cell_args] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
exec "$BINARY" "$@"
