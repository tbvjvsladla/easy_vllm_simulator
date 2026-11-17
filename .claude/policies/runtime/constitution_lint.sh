#!/bin/bash
# `constitution_lint.py [--repo R] [--format json|text] [--self-test]` through the node-built native executable (crate
# .claude/rust/policy-selftest). Direct normal ELF startup; no install/build/download and no Python fallback.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${CONSTITUTION_LINT_BIN:-$SDIR/../../rust/policy-selftest/target/release/constitution_lint}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[constitution_lint] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[constitution_lint] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[constitution_lint] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec -a "$SDIR/constitution_lint.sh" "$BIN" "$@"
