#!/bin/bash
# `runtime_selftest.py [--tripwires-only]` through the node-built native executable (crate .claude/rust/policy-selftest).
# Direct normal ELF startup; no install/build/download and no Python fallback: a missing executable is exit 2 with guidance.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${RUNTIME_SELFTEST_BIN:-$SDIR/../../rust/policy-selftest/target/release/runtime_selftest}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[runtime_selftest] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[runtime_selftest] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[runtime_selftest] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
# argv[0] = the original's path: the program derives the repository root from it (three parents up) and prints the
# original's program name in its usage text.
exec -a "$SDIR/runtime_selftest.sh" "$BIN" "$@"
