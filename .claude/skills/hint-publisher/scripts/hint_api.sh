#!/bin/bash
# JSON-lines library bridge of the hint-publisher Rust port (golden comparison of module functions; see hintlib::api).
# Explicit node-local build is required before execution; an absent binary is exit 2 (no Python fallback).
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${HINT_API_BIN:-$SDIR/../rust/target/release/hint_api}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[hint_api] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[hint_api] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
export HINT_SKILL_DIR="${HINT_SKILL_DIR:-$(cd "$SDIR/.." && pwd)}"
exec "$BINARY" "$@"
