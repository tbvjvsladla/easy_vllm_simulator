#!/bin/bash
# Native owner command. Node-local build is required before execution.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${PARSE_GUIDELLM_BIN:-$SDIR/../rust/target/release/parse_guidellm}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[parse_guidellm] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[parse_guidellm] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/build_rust.sh" >&2
  exit 2
fi
export PARSE_GUIDELLM_FIXTURE="${PARSE_GUIDELLM_FIXTURE:-$SDIR/../fixtures/guidellm_benchmarks_sample.json}"
exec "$BINARY" "$@"
