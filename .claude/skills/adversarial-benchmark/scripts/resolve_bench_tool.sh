#!/bin/bash
# Node-local Rust executable; installation and build are explicit operations.
set -euo pipefail
SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="${RESOLVE_BENCH_TOOL_BIN:-$SDIR/../rust/target/release/resolve_bench_tool}"
if [ ! -x "$BIN" ]; then
  echo '[resolve_bench_tool] ERROR Rust 실행파일이 없다. Terraform 설치·검사 뒤 명시 빌드가 필요하다.' >&2
  echo "  bash \"$SDIR/../../terraforming_node/scripts/rust_bootstrap.sh\" install" >&2
  echo "  bash \"$SDIR/build_rust.sh\" --offline" >&2
  exit 2
fi
export BENCH_TOOL_PIN_PATH="${BENCH_TOOL_PIN_PATH:-$SDIR/../bench_tool_pin.json}"
exec "$BIN" "$@"
