#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
BINARY="${BENCH_PRESET_BIN:-$SDIR/../rust/target/release/bench_preset}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[bench_preset] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[bench_preset] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
export BENCH_PRESET_REPO_ROOT="${BENCH_PRESET_REPO_ROOT:-$REPO}"
export BENCH_PRESET_SKILL_DIR="${BENCH_PRESET_SKILL_DIR:-$SDIR/..}"
# A caller-selected adapter is explicit legacy intent; empty keeps the old default meaning.
if [[ -n "${REPEAT_AXIS_CAMPAIGN_ADAPTER:-}" ]]; then
  export REPEAT_AXIS_CAMPAIGN_ADAPTER
else
  unset REPEAT_AXIS_CAMPAIGN_ADAPTER
fi
export REPEAT_AXIS_COMPAT_ADAPTER="${REPEAT_AXIS_COMPAT_ADAPTER:-$SDIR/repeat_axis_owner_query.py}"
exec "$BINARY" "$@"
