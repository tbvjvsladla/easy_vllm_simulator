#!/bin/bash
# `antipattern_scan.py --inventory|--tripwire|--self-test` through the node-built native executable (crate
# .claude/rust/policy-selftest). Direct normal ELF startup; no install/build/download and no Python fallback.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${ANTIPATTERN_SCAN_BIN:-$SDIR/../../rust/policy-selftest/target/release/antipattern_scan}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[antipattern_scan] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[antipattern_scan] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[antipattern_scan] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec -a "$SDIR/antipattern_scan.py" "$BIN" "$@"
