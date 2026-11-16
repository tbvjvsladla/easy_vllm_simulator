#!/bin/bash
# `python3 -m unittest runtime_patch_predicate` (runtime patch provenance suite; exit 0 all pass / 1 any failure, unittest-style
# summary on stderr) through the node-local Rust executable of `.claude/rust/policy-predicates`. Direct normal ELF startup:
# no install/build/download and no Python fallback.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${RUNTIME_PATCH_PREDICATE_BIN:-$SDIR/../../rust/policy-predicates/target/release/runtime_patch_predicate}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[runtime_patch_predicate] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[runtime_patch_predicate] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[runtime_patch_predicate] ERROR Rust 실행파일이 없다/실행 불가: %s (normal ELF 아님)\n' "$BIN" >&2
  printf '[runtime_patch_predicate] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
exec -a "$SDIR/runtime_patch_predicate.py" "$BIN" "$@"
