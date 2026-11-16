#!/bin/bash
# `claim_predicates.py` (run every registry clause predicate, JSON verdict on stdout, exit 0 PASS / 1 FAIL) through the
# node-local Rust executable of `.claude/rust/policy-predicates`. Direct normal ELF startup: no install/build/download and
# no Python fallback. A clause whose original still needs another bundle's Python in-process is reported `NotPorted`
# (a failure entry, never a pass). Extras: `--clause ID` (repeatable), `--list` (port status per clause).
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${CLAIM_PREDICATES_BIN:-$SDIR/../../rust/policy-predicates/target/release/claim_predicates}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[claim_predicates] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[claim_predicates] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[claim_predicates] ERROR Rust 실행파일이 없다/실행 불가: %s (normal ELF 아님)\n' "$BIN" >&2
  printf '[claim_predicates] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$SDIR/../../skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
exec -a "$SDIR/claim_predicates.py" "$BIN" "$@"
