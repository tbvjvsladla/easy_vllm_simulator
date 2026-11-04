#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
BINARY="${REPEAT_AXIS_API_BIN:-$SDIR/../rust/target/release/repeat_axis_api}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[repeat_axis] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[repeat_axis] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
export REPEAT_AXIS_REPO_ROOT="${REPEAT_AXIS_REPO_ROOT:-$REPO}"
export REPEAT_AXIS_CAMPAIGN_ADAPTER="${REPEAT_AXIS_CAMPAIGN_ADAPTER:-$SDIR/repeat_axis_owner_query.py}"
exec "$BINARY" "$@"
