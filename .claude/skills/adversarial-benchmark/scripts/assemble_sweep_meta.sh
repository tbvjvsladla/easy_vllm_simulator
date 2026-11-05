#!/bin/bash
# Explicit node-local build is required before execution.
set -euo pipefail
SELF_DIR="${BASH_SOURCE[0]%/*}"
if [ "$SELF_DIR" = "${BASH_SOURCE[0]}" ]; then SELF_DIR=.; fi
LAUNCH_DIR="$(cd -- "$SELF_DIR" && pwd)"
BINARY="${ASSEMBLE_SWEEP_META_BIN:-$LAUNCH_DIR/../rust/target/release/assemble_sweep_meta}"
if [ ! -f "$BINARY" ] || [ ! -x "$BINARY" ]; then
  printf '[assemble_sweep_meta] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BINARY" >&2
  printf '[assemble_sweep_meta] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$LAUNCH_DIR/build_rust.sh" >&2
  exit 2
fi
exec "$BINARY" "$@"
