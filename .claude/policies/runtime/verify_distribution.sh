#!/bin/bash
# `verify_distribution.py [--json-out PATH]` through the node-local Rust executable
# (.claude/rust/policy-distribution). Installation and build are explicit operations: a missing executable is an explicit
# exit 2 with guidance, never a Python start. argv[0] is the public .py path so usage/prog text and the repository root
# (`Path(__file__).resolve().parents[3]`) are the original's.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${VERIFY_DISTRIBUTION_BIN:-$SDIR/../../rust/policy-distribution/target/release/verify_distribution}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[verify_distribution] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[verify_distribution] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[verify_distribution] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec -a "$SDIR/verify_distribution.sh" "$BIN" "$@"
