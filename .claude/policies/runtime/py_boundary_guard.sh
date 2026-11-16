#!/bin/bash
# `py_boundary_guard check [...]` through the node-built native executable (crate .claude/rust/py-boundary-guard).
# plan_26111615 D5: 추적 *.py ⊆ .claude/policies/py_boundary_allowlist.tsv. 전수 = 완료 판정 · --staged-additions = pre-commit.
# Direct normal ELF startup; no install/build/download and no Python fallback: a missing executable is exit 2 with guidance.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${PY_BOUNDARY_GUARD_BIN:-$SDIR/../../rust/py-boundary-guard/target/release/py_boundary_guard}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[py_boundary_guard] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[py_boundary_guard] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[py_boundary_guard] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec "$BIN" "$@"
