#!/bin/bash
# bench_report reader/renderer (Rust port of render_bench_section.py). Explicit node-local build is required before
# execution; an absent binary is exit 2 with the build hint (no Python fallback).
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${RENDER_BENCH_SECTION_BIN:-$SDIR/../rust/target/release/render_bench_section}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[render_bench_section] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[render_bench_section] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
export HINT_SKILL_DIR="${HINT_SKILL_DIR:-$(cd "$SDIR/.." && pwd)}"
exec "$BINARY" "$@"
