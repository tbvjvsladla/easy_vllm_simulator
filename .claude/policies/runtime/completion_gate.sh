#!/bin/bash
# `completion_gate.py verify|authorize ...` through the node-local Rust executable (crate .claude/rust/policy-gate).
# Installation and build are explicit operations: a missing executable is exit 2 with guidance, never a Python start.
# argv[0] is the original script path so the program locates the repo/schemas exactly as `Path(__file__)` did and its
# argparse-style usage texts name `completion_gate.py`.
set -euo pipefail
SELF="${BASH_SOURCE[0]}"
case "$SELF" in */*) SDIR_RAW="${SELF%/*}"; [ -n "$SDIR_RAW" ] || SDIR_RAW=/ ;; *) SDIR_RAW=. ;; esac
SDIR="$(cd "$SDIR_RAW" && pwd -P)"
BIN="${COMPLETION_GATE_BIN:-$SDIR/../../rust/policy-gate/target/release/completion_gate}"
if [ ! -f "$BIN" ] || [ ! -x "$BIN" ]; then
  printf '[completion_gate] ERROR Rust 실행파일이 없다/실행 불가: %s\n' "$BIN" >&2
  printf '[completion_gate] 먼저 Terraform Rust install/check 후 실행: bash %q\n' "$(cd "$SDIR/../../.." && pwd -P)/.claude/skills/terraforming_node/scripts/build_rust_packages.sh" >&2
  exit 2
fi
magic=''
IFS= read -r -N 4 magic < "$BIN" || :
if [ "$magic" != $'\177ELF' ]; then
  printf '[completion_gate] ERROR normal Rust ELF invalid: %s\n' "$BIN" >&2
  exit 2
fi
exec -a "$SDIR/completion_gate.sh" "$BIN" "$@"
